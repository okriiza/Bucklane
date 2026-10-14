mod body;
mod download;
mod upload;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{SecondsFormat, Utc};
use sqlx::SqlitePool;
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use crate::domain::bucket::validate_bucket_name;
use crate::domain::object::ObjectMetadata;
use crate::domain::transfer::{
    EnqueueDownloadInput, EnqueueItemResult, EnqueueUploadInput, StoredTransfer,
    TransferConflictPolicy, TransferDirection, TransferDto, TransferStatus,
};
use crate::errors::{AppError, ErrorCode};
use crate::events::TransferUpdate;
use crate::providers::s3_client_factory::ClientCache;
use crate::providers::s3_provider::S3StorageProvider;
use crate::providers::storage_provider::StorageProvider;
use crate::repositories::transfer_repository::TransferRepository;
use crate::security::credential_store::CredentialStore;
use crate::security::local_selection::{
    SelectionKind, SelectionRegistry, join_in_dir, numbered_name, safe_local_file_name,
};
use crate::services::connection_service::ConnectionService;

/// Default number of transfers running at once (PRD §9.1).
const MAX_CONCURRENT_TRANSFERS: usize = 3;
/// Progress events are throttled so large transfers don't flood the UI.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(300);
/// Progress is persisted less often than it is emitted.
const PERSIST_EVERY_TICKS: u32 = 7;
/// `name (1).ext` … `name (999).ext` are tried before giving up.
const MAX_RENAME_ATTEMPTS: u32 = 999;

/// Runs uploads and downloads as managed background tasks (TSD §9).
///
/// Each transfer task is the only writer of its database row while active,
/// and the only one to decide its terminal status, so completion and
/// cancellation can never produce contradictory states.
#[derive(Clone)]
pub struct TransferManager {
    inner: Arc<Inner>,
}

struct Inner {
    app: AppHandle,
    pool: SqlitePool,
    repo: TransferRepository,
    credentials: Arc<dyn CredentialStore>,
    clients: Arc<ClientCache>,
    semaphore: Arc<Semaphore>,
    active: Mutex<HashMap<String, ActiveTransfer>>,
}

struct ActiveTransfer {
    connection_id: Option<String>,
    token: CancellationToken,
}

/// Bytes transferred so far; uploads keep one counter per multipart part.
#[derive(Default)]
pub(crate) struct Progress {
    counters: Mutex<Vec<Arc<AtomicU64>>>,
}

impl Progress {
    pub(crate) fn counter(&self) -> Arc<AtomicU64> {
        let counter = Arc::new(AtomicU64::new(0));
        self.counters
            .lock()
            .expect("progress poisoned")
            .push(counter.clone());
        counter
    }

    fn total(&self) -> i64 {
        let sum: u64 = self
            .counters
            .lock()
            .expect("progress poisoned")
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .sum();
        i64::try_from(sum).unwrap_or(i64::MAX)
    }
}

enum Outcome {
    Completed,
    Failed(AppError),
    Canceled,
}

impl TransferManager {
    pub fn new(
        app: AppHandle,
        pool: SqlitePool,
        credentials: Arc<dyn CredentialStore>,
        clients: Arc<ClientCache>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                app,
                repo: TransferRepository::new(pool.clone()),
                pool,
                credentials,
                clients,
                semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_TRANSFERS)),
                active: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub async fn list(&self) -> Result<Vec<TransferDto>, AppError> {
        Ok(self
            .inner
            .repo
            .list_recent()
            .await?
            .iter()
            .map(TransferDto::from)
            .collect())
    }

    pub async fn get(&self, id: &str) -> Result<TransferDto, AppError> {
        self.inner
            .repo
            .get(id)
            .await?
            .as_ref()
            .map(TransferDto::from)
            .ok_or_else(not_found)
    }

    pub fn has_active(&self, connection_id: &str) -> bool {
        self.inner
            .active
            .lock()
            .expect("active transfers poisoned")
            .values()
            .any(|a| a.connection_id.as_deref() == Some(connection_id))
    }

    /// Requests cancellation; the transfer task performs cleanup and records
    /// the final status.
    pub fn cancel(&self, id: &str) -> Result<(), AppError> {
        let active = self.inner.active.lock().expect("active transfers poisoned");
        let transfer = active.get(id).ok_or_else(|| {
            AppError::new(
                ErrorCode::InvalidInput,
                "This transfer is no longer active.",
            )
        })?;
        transfer.token.cancel();
        Ok(())
    }

    /// FAILED → QUEUED on the same row (TSD §6.5.2). A multipart upload is
    /// restarted from scratch after aborting the previous attempt.
    pub async fn retry(&self, id: &str) -> Result<TransferDto, AppError> {
        let mut t = self.inner.repo.get(id).await?.ok_or_else(not_found)?;
        if t.status != TransferStatus::Failed {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "Only failed transfers can be retried.",
            ));
        }
        if t.connection_id.is_none() {
            return Err(AppError::new(
                ErrorCode::ConnectionInvalid,
                "The connection for this transfer was deleted.",
            ));
        }
        self.inner.abort_multipart(&mut t).await;

        t.status = TransferStatus::Queued;
        t.bytes_transferred = 0;
        t.retry_count += 1;
        t.error_code = None;
        t.error_message = None;
        t.started_at = None;
        t.completed_at = None;
        self.inner.repo.update(&t).await?;
        let dto = TransferDto::from(&t);
        self.inner.emit(&t);
        self.spawn(t);
        Ok(dto)
    }

    pub async fn clear_history(&self) -> Result<u64, AppError> {
        self.inner.repo.clear_history().await
    }

    pub async fn enqueue_upload(
        &self,
        input: EnqueueUploadInput,
        selections: &SelectionRegistry,
    ) -> Result<Vec<EnqueueItemResult>, AppError> {
        let bucket = validate_bucket_name(&input.bucket)?.to_owned();
        if !input.prefix.is_empty() && !input.prefix.ends_with('/') {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "The destination folder is not valid.",
            ));
        }
        let connections = self.inner.connections();
        let connection_name = connections.get(&input.connection_id).await?.name;
        let provider = S3StorageProvider::new(
            connections
                .client_for_bucket(&input.connection_id, &bucket)
                .await?,
        );

        let mut results = Vec::with_capacity(input.selection_ids.len());
        for selection_id in &input.selection_ids {
            let selection = match selections.get(selection_id, SelectionKind::File) {
                Ok(s) => s,
                Err(error) => {
                    results.push(rejected(selection_id, selection_id, error));
                    continue;
                }
            };
            let name = selection
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let size = std::fs::metadata(&selection.path).map(|m| m.len()).ok();

            let mut key = format!("{}{name}", input.prefix);
            let mut overwrite = false;
            match remote_exists(&provider, &bucket, &key).await {
                Err(error) => {
                    results.push(rejected(selection_id, &name, error));
                    continue;
                }
                Ok(false) => {}
                Ok(true) => match input.conflict_policy {
                    TransferConflictPolicy::Ask => {
                        results.push(EnqueueItemResult::Conflict {
                            item: selection_id.clone(),
                            name,
                        });
                        continue;
                    }
                    TransferConflictPolicy::Skip => {
                        results.push(EnqueueItemResult::Skipped {
                            item: selection_id.clone(),
                            name,
                        });
                        continue;
                    }
                    TransferConflictPolicy::Overwrite => overwrite = true,
                    TransferConflictPolicy::Rename => {
                        match free_remote_key(&provider, &bucket, &input.prefix, &name).await {
                            Ok(free) => key = free,
                            Err(error) => {
                                results.push(rejected(selection_id, &name, error));
                                continue;
                            }
                        }
                    }
                },
            }

            let transfer = self
                .create(
                    &input.connection_id,
                    &connection_name,
                    TransferDirection::Upload,
                    &bucket,
                    &key,
                    &selection.path.to_string_lossy(),
                    size.and_then(|s| i64::try_from(s).ok()),
                    overwrite,
                )
                .await?;
            results.push(EnqueueItemResult::Queued {
                transfer_id: transfer,
                key,
            });
        }
        Ok(results)
    }

    pub async fn enqueue_download(
        &self,
        input: EnqueueDownloadInput,
        selections: &SelectionRegistry,
    ) -> Result<Vec<EnqueueItemResult>, AppError> {
        let bucket = validate_bucket_name(&input.bucket)?.to_owned();
        let dir = selections
            .get(&input.directory_selection_id, SelectionKind::Directory)?
            .path;
        let connection_name = self
            .inner
            .connections()
            .get(&input.connection_id)
            .await?
            .name;

        let mut results = Vec::with_capacity(input.keys.len());
        for key in &input.keys {
            let base = key.rsplit('/').next().unwrap_or(key);
            if key.ends_with('/') {
                results.push(rejected(
                    key,
                    key,
                    AppError::new(
                        ErrorCode::OperationUnsupported,
                        "Downloading folders is not supported yet.",
                    ),
                ));
                continue;
            }
            let Some(file_name) = safe_local_file_name(base) else {
                results.push(rejected(
                    key,
                    base,
                    AppError::new(
                        ErrorCode::LocalFileError,
                        "This object's name cannot be used as a file name.",
                    ),
                ));
                continue;
            };
            let Some(mut path) = join_in_dir(&dir, &file_name) else {
                results.push(rejected(
                    key,
                    base,
                    AppError::new(ErrorCode::LocalFileError, "Invalid destination."),
                ));
                continue;
            };

            let mut overwrite = false;
            if path.exists() {
                match input.conflict_policy {
                    TransferConflictPolicy::Ask => {
                        results.push(EnqueueItemResult::Conflict {
                            item: key.clone(),
                            name: file_name,
                        });
                        continue;
                    }
                    TransferConflictPolicy::Skip => {
                        results.push(EnqueueItemResult::Skipped {
                            item: key.clone(),
                            name: file_name,
                        });
                        continue;
                    }
                    TransferConflictPolicy::Overwrite => overwrite = true,
                    TransferConflictPolicy::Rename => {
                        match (1..=MAX_RENAME_ATTEMPTS)
                            .filter_map(|n| join_in_dir(&dir, &numbered_name(&file_name, n)))
                            .find(|p| !p.exists())
                        {
                            Some(free) => path = free,
                            None => {
                                results.push(rejected(key, base, no_free_name()));
                                continue;
                            }
                        }
                    }
                }
            }

            let transfer = self
                .create(
                    &input.connection_id,
                    &connection_name,
                    TransferDirection::Download,
                    &bucket,
                    key,
                    &path.to_string_lossy(),
                    None,
                    overwrite,
                )
                .await?;
            results.push(EnqueueItemResult::Queued {
                transfer_id: transfer,
                key: key.clone(),
            });
        }
        Ok(results)
    }

    /// Reconciles transfers left unfinished by a previous session (TSD §9.2):
    /// they become FAILED/INTERRUPTED. Nothing is resumed or overwritten.
    pub async fn reconcile_on_startup(&self) {
        let inner = self.inner.clone();
        tauri::async_runtime::spawn(async move {
            let unfinished = match inner.repo.unfinished().await {
                Ok(rows) => rows,
                Err(_) => return,
            };
            for mut t in unfinished {
                if t.direction == TransferDirection::Download {
                    download::remove_temp_file(&t).await;
                }
                t.status = TransferStatus::Failed;
                t.error_code = Some("INTERRUPTED".to_owned());
                t.error_message =
                    Some("Bucklane was closed before this transfer finished.".to_owned());
                t.completed_at = Some(now());
                let _ = inner.repo.update(&t).await;
            }

            // Best-effort cleanup of multipart uploads that could not be
            // aborted earlier (DB design §6.4).
            if let Ok(orphans) = inner.repo.with_orphaned_uploads().await {
                for mut t in orphans {
                    inner.abort_multipart(&mut t).await;
                    let _ = inner.repo.update(&t).await;
                }
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    async fn create(
        &self,
        connection_id: &str,
        connection_name: &str,
        direction: TransferDirection,
        bucket: &str,
        key: &str,
        local_path: &str,
        bytes_total: Option<i64>,
        overwrite: bool,
    ) -> Result<String, AppError> {
        let t = StoredTransfer {
            id: uuid::Uuid::now_v7().to_string(),
            connection_id: Some(connection_id.to_owned()),
            connection_name_snapshot: connection_name.to_owned(),
            direction,
            bucket: bucket.to_owned(),
            object_key: key.to_owned(),
            local_path: local_path.to_owned(),
            bytes_total,
            bytes_transferred: 0,
            status: TransferStatus::Queued,
            retry_count: 0,
            error_code: None,
            error_message: None,
            multipart_upload_id: None,
            overwrite,
            created_at: now(),
            started_at: None,
            completed_at: None,
        };
        self.inner.repo.insert(&t).await?;
        let id = t.id.clone();
        self.inner.emit(&t);
        self.spawn(t);
        Ok(id)
    }

    fn spawn(&self, t: StoredTransfer) {
        let token = CancellationToken::new();
        self.inner
            .active
            .lock()
            .expect("active transfers poisoned")
            .insert(
                t.id.clone(),
                ActiveTransfer {
                    connection_id: t.connection_id.clone(),
                    token: token.clone(),
                },
            );
        let inner = self.inner.clone();
        tauri::async_runtime::spawn(async move { inner.run(t, token).await });
    }
}

impl Inner {
    fn connections(&self) -> ConnectionService<'_> {
        ConnectionService::new(&self.pool, self.credentials.as_ref(), &self.clients)
    }

    fn emit(&self, t: &StoredTransfer) {
        let _ = TransferUpdate(TransferDto::from(t)).emit(&self.app);
    }

    async fn run(self: Arc<Self>, mut t: StoredTransfer, token: CancellationToken) {
        // Wait for a slot; a queued transfer can be canceled while waiting.
        let permit = tokio::select! {
            permit = self.semaphore.clone().acquire_owned() => permit.ok(),
            () = token.cancelled() => None,
        };
        let outcome = match permit {
            None => Outcome::Canceled,
            Some(_permit) => {
                t.status = TransferStatus::Running;
                t.started_at = Some(now());
                t.bytes_transferred = 0;
                if let Err(err) = self.repo.update(&t).await {
                    tracing::error!(error = %err, "failed to mark transfer running");
                }
                self.emit(&t);
                tracing::info!(transfer_id = %t.id, direction = ?t.direction, "transfer started");

                let progress = Arc::new(Progress::default());
                let reporter = self.clone().report_progress(t.clone(), progress.clone());
                let result = tokio::select! {
                    r = self.execute(&mut t, &progress) => r.map_or_else(Outcome::Failed, |()| Outcome::Completed),
                    () = token.cancelled() => Outcome::Canceled,
                };
                reporter.abort();
                // Make sure no progress event can follow the final status.
                let _ = reporter.await;
                t.bytes_transferred = progress.total();
                result
            }
        };

        // Reload: the task may have recorded a multipart upload ID.
        if let Ok(Some(saved)) = self.repo.get(&t.id).await {
            t.multipart_upload_id = saved.multipart_upload_id;
            t.bytes_total = t.bytes_total.or(saved.bytes_total);
        }
        if !matches!(outcome, Outcome::Completed) {
            self.cleanup(&mut t).await;
        }

        t.completed_at = Some(now());
        match outcome {
            Outcome::Completed => {
                t.status = TransferStatus::Completed;
                if let Some(total) = t.bytes_total {
                    t.bytes_transferred = total;
                }
            }
            Outcome::Canceled => {
                t.status = TransferStatus::Canceled;
                t.error_code = Some("TRANSFER_CANCELED".to_owned());
                t.error_message = Some("Canceled.".to_owned());
            }
            Outcome::Failed(err) => {
                t.status = TransferStatus::Failed;
                t.error_code = Some(err.code_str());
                t.error_message = Some(err.message.clone());
            }
        }
        if let Err(err) = self.repo.update(&t).await {
            tracing::error!(error = %err, "failed to record transfer result");
        }
        self.active
            .lock()
            .expect("active transfers poisoned")
            .remove(&t.id);
        tracing::info!(transfer_id = %t.id, status = ?t.status, "transfer finished");
        self.emit(&t);
    }

    async fn execute(&self, t: &mut StoredTransfer, progress: &Progress) -> Result<(), AppError> {
        let connection_id = t.connection_id.clone().ok_or_else(|| {
            AppError::new(
                ErrorCode::ConnectionInvalid,
                "The connection for this transfer was deleted.",
            )
        })?;
        let client = self
            .connections()
            .client_for_bucket(&connection_id, &t.bucket)
            .await?;
        match t.direction {
            TransferDirection::Upload => upload::run(&client, &self.repo, t, progress).await,
            TransferDirection::Download => download::run(&client, &self.repo, t, progress).await,
        }
    }

    async fn cleanup(&self, t: &mut StoredTransfer) {
        match t.direction {
            TransferDirection::Download => download::remove_temp_file(t).await,
            TransferDirection::Upload => self.abort_multipart(t).await,
        }
    }

    /// Aborts a recorded multipart upload. On failure the upload ID is kept
    /// so cleanup can be retried at the next startup.
    async fn abort_multipart(&self, t: &mut StoredTransfer) {
        let (Some(upload_id), Some(connection_id)) =
            (t.multipart_upload_id.clone(), t.connection_id.clone())
        else {
            return;
        };
        let Ok(client) = self
            .connections()
            .client_for_bucket(&connection_id, &t.bucket)
            .await
        else {
            return;
        };
        match upload::abort(&client, &t.bucket, &t.object_key, &upload_id).await {
            Ok(()) => {
                t.multipart_upload_id = None;
                let _ = self.repo.delete_parts(&t.id).await;
            }
            Err(err) => {
                tracing::warn!(transfer_id = %t.id, error = %err, "multipart abort failed; will retry at startup");
            }
        }
    }

    fn report_progress(
        self: Arc<Self>,
        mut snapshot: StoredTransfer,
        progress: Arc<Progress>,
    ) -> tauri::async_runtime::JoinHandle<()> {
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(PROGRESS_INTERVAL);
            let mut ticks = 0u32;
            loop {
                interval.tick().await;
                snapshot.bytes_transferred = progress.total();
                self.emit(&snapshot);
                ticks += 1;
                // Pick up bytes_total once the task has measured it.
                if snapshot.bytes_total.is_none()
                    && let Ok(Some(saved)) = self.repo.get(&snapshot.id).await
                {
                    snapshot.bytes_total = saved.bytes_total;
                }
                if ticks.is_multiple_of(PERSIST_EVERY_TICKS) {
                    let _ = self
                        .repo
                        .update_progress(&snapshot.id, snapshot.bytes_transferred)
                        .await;
                }
            }
        })
    }
}

async fn remote_exists(
    provider: &S3StorageProvider,
    bucket: &str,
    key: &str,
) -> Result<bool, AppError> {
    match provider.head_object(bucket, key).await {
        Ok(ObjectMetadata { .. }) => Ok(true),
        Err(err) if err.code == ErrorCode::ObjectNotFound => Ok(false),
        Err(err) => Err(err),
    }
}

async fn free_remote_key(
    provider: &S3StorageProvider,
    bucket: &str,
    prefix: &str,
    name: &str,
) -> Result<String, AppError> {
    for n in 1..=MAX_RENAME_ATTEMPTS {
        let candidate = format!("{prefix}{}", numbered_name(name, n));
        if !remote_exists(provider, bucket, &candidate).await? {
            return Ok(candidate);
        }
    }
    Err(no_free_name())
}

fn rejected(item: &str, name: &str, error: AppError) -> EnqueueItemResult {
    EnqueueItemResult::Rejected {
        item: item.to_owned(),
        name: name.to_owned(),
        error,
    }
}

fn no_free_name() -> AppError {
    AppError::new(
        ErrorCode::InvalidInput,
        "Could not find a free name for the renamed copy.",
    )
}

fn not_found() -> AppError {
    AppError::new(ErrorCode::InvalidInput, "The transfer no longer exists.")
}

pub(crate) fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Upload/download round trips against an S3-compatible server when `BUCKLANE_S3_TEST_ENDPOINT`
/// is set; skipped otherwise.
#[cfg(test)]
mod s3_integration_tests {
    use std::path::PathBuf;

    use super::*;
    use crate::database::test_pool;
    use crate::domain::connection::ProviderType;
    use crate::domain::connection::ResolvedConfig;
    use crate::providers::s3_client_factory::build_client;
    use crate::security::SecretString;

    struct Fixture {
        repo: TransferRepository,
        client: aws_sdk_s3::Client,
        bucket: String,
        dir: PathBuf,
    }

    impl Fixture {
        async fn new() -> Option<Self> {
            let Ok(endpoint) = std::env::var("BUCKLANE_S3_TEST_ENDPOINT") else {
                eprintln!("skipped: BUCKLANE_S3_TEST_ENDPOINT not set");
                return None;
            };
            let client = build_client(
                &ResolvedConfig {
                    provider: ProviderType::Minio,
                    endpoint: Some(endpoint),
                    region: "us-east-1".into(),
                    access_key_id: "bucklane-test".into(),
                    force_path_style: true,
                    allow_insecure_http: true,
                },
                &SecretString::new("bucklane-test-secret"),
            );
            let bucket = format!("bucklane-xfer-{}", uuid::Uuid::now_v7().simple());
            client.create_bucket().bucket(&bucket).send().await.unwrap();
            let dir = std::env::temp_dir().join(format!("bucklane-xfer-{}", uuid::Uuid::now_v7()));
            std::fs::create_dir_all(&dir).unwrap();
            Some(Self {
                repo: TransferRepository::new(test_pool().await),
                client,
                bucket,
                dir,
            })
        }

        async fn transfer(
            &self,
            direction: TransferDirection,
            key: &str,
            local: &std::path::Path,
            overwrite: bool,
        ) -> StoredTransfer {
            let t = StoredTransfer {
                id: uuid::Uuid::now_v7().to_string(),
                connection_id: None,
                connection_name_snapshot: "MinIO".into(),
                direction,
                bucket: self.bucket.clone(),
                object_key: key.into(),
                local_path: local.to_string_lossy().into_owned(),
                bytes_total: None,
                bytes_transferred: 0,
                status: TransferStatus::Running,
                retry_count: 0,
                error_code: None,
                error_message: None,
                multipart_upload_id: None,
                overwrite,
                created_at: now(),
                started_at: None,
                completed_at: None,
            };
            self.repo.insert(&t).await.unwrap();
            t
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[tokio::test]
    async fn small_file_round_trip() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        let source = f.dir.join("hello.txt");
        std::fs::write(&source, b"hello bucklane").unwrap();

        let mut up = f
            .transfer(TransferDirection::Upload, "docs/hello.txt", &source, false)
            .await;
        let progress = Progress::default();
        upload::run(&f.client, &f.repo, &mut up, &progress)
            .await
            .unwrap();
        assert_eq!(progress.total(), 14);

        let target = f.dir.join("copy.txt");
        let mut down = f
            .transfer(
                TransferDirection::Download,
                "docs/hello.txt",
                &target,
                false,
            )
            .await;
        download::run(&f.client, &f.repo, &mut down, &Progress::default())
            .await
            .unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"hello bucklane");
        assert!(download::temp_path(&down).is_some_and(|p| !p.exists()));
    }

    #[tokio::test]
    async fn large_file_uses_multipart_and_cleans_up_parts() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        let source = f.dir.join("big.bin");
        let data: Vec<u8> = (0..70 * 1024 * 1024u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&source, &data).unwrap();

        let mut up = f
            .transfer(TransferDirection::Upload, "big.bin", &source, false)
            .await;
        let progress = Progress::default();
        upload::run(&f.client, &f.repo, &mut up, &progress)
            .await
            .unwrap();

        assert_eq!(progress.total(), data.len() as i64);
        assert_eq!(
            up.multipart_upload_id, None,
            "upload id cleared after completion"
        );
        let target = f.dir.join("big-copy.bin");
        let mut down = f
            .transfer(TransferDirection::Download, "big.bin", &target, false)
            .await;
        download::run(&f.client, &f.repo, &mut down, &Progress::default())
            .await
            .unwrap();
        assert!(
            std::fs::read(&target).unwrap() == data,
            "downloaded bytes differ"
        );
    }

    #[tokio::test]
    async fn upload_refuses_to_replace_without_overwrite() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        let source = f.dir.join("a.txt");
        std::fs::write(&source, b"first").unwrap();
        let mut first = f
            .transfer(TransferDirection::Upload, "a.txt", &source, false)
            .await;
        upload::run(&f.client, &f.repo, &mut first, &Progress::default())
            .await
            .unwrap();

        std::fs::write(&source, b"second").unwrap();
        let mut second = f
            .transfer(TransferDirection::Upload, "a.txt", &source, false)
            .await;
        let err = upload::run(&f.client, &f.repo, &mut second, &Progress::default())
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::TransferFailed);
    }
}
