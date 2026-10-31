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
use crate::domain::settings::AppSettings;
use crate::domain::transfer::{
    EnqueueDownloadInput, EnqueueItemResult, EnqueueUploadInput, StoredTransfer,
    TransferConflictPolicy, TransferDirection, TransferDto, TransferStatus,
};
use crate::errors::{AppError, ErrorCode};
use crate::events::TransferUpdate;
use crate::providers::s3_client_factory::ClientCache;
use crate::providers::s3_provider::S3StorageProvider;
use crate::providers::storage_provider::ListQuery;
use crate::providers::storage_provider::StorageProvider;
use crate::repositories::transfer_repository::TransferRepository;
use crate::security::credential_store::CredentialStore;
use crate::security::local_selection::{
    MAX_FOLDER_FILES, SelectionKind, SelectionRegistry, collect_files, join_in_dir, join_relative,
    numbered_name, safe_local_file_name, stays_within,
};
use crate::services::connection_service::ConnectionService;

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
    /// Permits currently granted to the semaphore (the concurrency limit).
    limit: Mutex<usize>,
    tuning: Mutex<upload::UploadTuning>,
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
        let defaults = AppSettings::default();
        let limit = defaults.max_concurrent_transfers as usize;
        Self {
            inner: Arc::new(Inner {
                app,
                repo: TransferRepository::new(pool.clone()),
                pool,
                credentials,
                clients,
                semaphore: Arc::new(Semaphore::new(limit)),
                limit: Mutex::new(limit),
                tuning: Mutex::new(upload::UploadTuning::from_settings(&defaults)),
                active: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Applies transfer settings at runtime. Lowering the limit never stops
    /// running transfers; the surplus slots are withdrawn as they free up.
    pub fn apply_settings(&self, settings: &AppSettings) {
        *self.inner.tuning.lock().expect("tuning poisoned") =
            upload::UploadTuning::from_settings(settings);

        let new_limit = settings.max_concurrent_transfers as usize;
        let mut limit = self.inner.limit.lock().expect("limit poisoned");
        if new_limit > *limit {
            self.inner.semaphore.add_permits(new_limit - *limit);
        } else if new_limit < *limit {
            let surplus = u32::try_from(*limit - new_limit).unwrap_or(u32::MAX);
            let semaphore = self.inner.semaphore.clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(permits) = semaphore.acquire_many_owned(surplus).await {
                    permits.forget();
                }
            });
        }
        *limit = new_limit;
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

    /// FAILED → QUEUED on the same row (TSD §6.5.2). A download continues
    /// from its last checkpoint when possible; a multipart upload is restarted
    /// from scratch after aborting the previous attempt.
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

    /// Shows a transfer's local file in Finder / File Explorer. The path
    /// never leaves the backend; the frontend only names the transfer.
    pub async fn reveal(&self, id: &str) -> Result<(), AppError> {
        let t = self.inner.repo.get(id).await?.ok_or_else(not_found)?;
        let path = std::path::PathBuf::from(&t.local_path);
        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Err(AppError::new(
                ErrorCode::LocalFileError,
                "The file is no longer at its original location.",
            ));
        }
        tauri::async_runtime::spawn_blocking(move || tauri_plugin_opener::reveal_item_in_dir(path))
            .await
            .map_err(|_| AppError::internal("Opening the file manager was interrupted."))?
            .map_err(|err| {
                AppError::new(
                    ErrorCode::LocalFileError,
                    "The file manager could not be opened.",
                )
                .with_details(err.to_string())
            })
    }

    /// Also deletes the temporary files kept for resuming failed downloads.
    pub async fn clear_history(&self) -> Result<u64, AppError> {
        for t in self.inner.repo.failed_downloads().await? {
            download::remove_temp_file(&t).await;
        }
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

        let mut results = Vec::new();
        let mut planned = Vec::new();
        for item in &input.selection_ids {
            match plan_upload(&provider, &bucket, &input.prefix, item, selections).await {
                Ok(items) => planned.extend(items),
                Err(error) => results.push(rejected(item, item, error)),
            }
        }

        for plan in planned {
            let mut key = plan.key;
            let mut overwrite = false;
            let exists = match plan.exists {
                Some(known) => Ok(known),
                None => remote_exists(&provider, &bucket, &key).await,
            };
            match exists {
                Err(error) => {
                    results.push(rejected(&plan.item, &plan.name, error));
                    continue;
                }
                Ok(false) => {}
                Ok(true) => match input.conflict_policy {
                    TransferConflictPolicy::Ask => {
                        results.push(EnqueueItemResult::Conflict {
                            item: plan.item,
                            name: plan.name,
                        });
                        continue;
                    }
                    TransferConflictPolicy::Skip => {
                        results.push(EnqueueItemResult::Skipped {
                            item: plan.item,
                            name: plan.name,
                        });
                        continue;
                    }
                    TransferConflictPolicy::Overwrite => overwrite = true,
                    TransferConflictPolicy::Rename => {
                        match free_remote_key(&provider, &bucket, &key).await {
                            Ok(free) => key = free,
                            Err(error) => {
                                results.push(rejected(&plan.item, &plan.name, error));
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
                    &plan.path.to_string_lossy(),
                    plan.size,
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
        let connections = self.inner.connections();
        let connection_name = connections.get(&input.connection_id).await?.name;
        let provider = S3StorageProvider::new(
            connections
                .client_for_bucket(&input.connection_id, &bucket)
                .await?,
        );

        let mut results = Vec::new();
        let mut planned = Vec::new();
        for item in &input.keys {
            match plan_download(&provider, &bucket, &dir, item).await {
                Ok(items) => planned.extend(items),
                Err(error) => results.push(rejected(item, item, error)),
            }
        }

        for plan in planned {
            let mut path = plan.path;
            let mut overwrite = false;
            if path.exists() {
                match input.conflict_policy {
                    TransferConflictPolicy::Ask => {
                        results.push(EnqueueItemResult::Conflict {
                            item: plan.item,
                            name: plan.name,
                        });
                        continue;
                    }
                    TransferConflictPolicy::Skip => {
                        results.push(EnqueueItemResult::Skipped {
                            item: plan.item,
                            name: plan.name,
                        });
                        continue;
                    }
                    TransferConflictPolicy::Overwrite => overwrite = true,
                    TransferConflictPolicy::Rename => match free_local_path(&path) {
                        Some(free) => path = free,
                        None => {
                            results.push(rejected(&plan.item, &plan.name, no_free_name()));
                            continue;
                        }
                    },
                }
            }

            let transfer = self
                .create(
                    &input.connection_id,
                    &connection_name,
                    TransferDirection::Download,
                    &bucket,
                    &plan.key,
                    &path.to_string_lossy(),
                    plan.size,
                    overwrite,
                )
                .await?;
            results.push(EnqueueItemResult::Queued {
                transfer_id: transfer,
                key: plan.key,
            });
        }
        Ok(results)
    }

    /// Reconciles transfers left unfinished by a previous session (TSD §9.2):
    /// they become FAILED/INTERRUPTED. Nothing is resumed or overwritten.
    /// Marks transfers left over from the last run as interrupted. Downloads
    /// keep their temporary file; with `auto_resume` they continue at once.
    pub async fn reconcile_on_startup(&self, auto_resume: bool) {
        let manager = self.clone();
        let inner = self.inner.clone();
        tauri::async_runtime::spawn(async move {
            let unfinished = match inner.repo.unfinished().await {
                Ok(rows) => rows,
                Err(_) => return,
            };
            let mut to_resume = Vec::new();
            for mut t in unfinished {
                let resumable = t.direction == TransferDirection::Download && t.resume_offset > 0;
                t.status = TransferStatus::Failed;
                t.error_code = Some("INTERRUPTED".to_owned());
                t.error_message = Some(
                    if resumable {
                        "Bucklane was closed before this download finished. Resume to continue where it stopped."
                    } else {
                        "Bucklane was closed before this transfer finished."
                    }
                    .to_owned(),
                );
                t.completed_at = Some(now());
                let _ = inner.repo.update(&t).await;
                if resumable {
                    to_resume.push(t.id);
                }
            }
            if auto_resume {
                for id in to_resume {
                    if let Err(err) = manager.retry(&id).await {
                        tracing::warn!(transfer_id = %id, error = %err, "automatic resume failed");
                    }
                }
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
            resume_etag: None,
            resume_offset: 0,
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
                // A resuming download starts at its checkpoint, so the speed
                // estimate doesn't see the earlier bytes as an instant jump.
                t.bytes_transferred = match t.direction {
                    TransferDirection::Download => t.resume_offset,
                    TransferDirection::Upload => 0,
                };
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
        match outcome {
            Outcome::Completed => {}
            // A failed download keeps its temporary file so it can resume.
            Outcome::Failed(_) if t.direction == TransferDirection::Download => {}
            Outcome::Failed(_) | Outcome::Canceled => self.cleanup(&mut t).await,
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
            TransferDirection::Upload => {
                let tuning = *self.tuning.lock().expect("tuning poisoned");
                upload::run(&client, &self.repo, t, progress, tuning).await
            }
            TransferDirection::Download => download::run(&client, &self.repo, t, progress).await,
        }
    }

    async fn cleanup(&self, t: &mut StoredTransfer) {
        match t.direction {
            TransferDirection::Download => {
                download::remove_temp_file(t).await;
                t.resume_etag = None;
                t.resume_offset = 0;
            }
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

/// `folder/name.ext` → `folder/name (1).ext`, `folder/name (2).ext`, …
async fn free_remote_key(
    provider: &S3StorageProvider,
    bucket: &str,
    key: &str,
) -> Result<String, AppError> {
    let split = key.rfind('/').map_or(0, |i| i + 1);
    let (prefix, name) = key.split_at(split);
    for n in 1..=MAX_RENAME_ATTEMPTS {
        let candidate = format!("{prefix}{}", numbered_name(name, n));
        if !remote_exists(provider, bucket, &candidate).await? {
            return Ok(candidate);
        }
    }
    Err(no_free_name())
}

fn free_local_path(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let dir = path.parent()?;
    let name = path.file_name()?.to_string_lossy().into_owned();
    (1..=MAX_RENAME_ATTEMPTS)
        .filter_map(|n| join_in_dir(dir, &numbered_name(&name, n)))
        .find(|p| !p.exists())
}

/// Separates a folder selection (or folder key) from one file inside it in
/// conflict item IDs, so a resend targets only that file. A unit separator
/// does not occur in file names or real-world object keys.
const ITEM_SEPARATOR: char = '\u{1f}';
/// Keys gathered to check for existing objects before a folder upload; past
/// this, remaining files fall back to one HEAD request each.
const MAX_EXISTING_KEYS: usize = 100_000;

struct UploadPlan {
    item: String,
    name: String,
    path: std::path::PathBuf,
    key: String,
    size: Option<i64>,
    /// `None` when unknown; checked with HEAD before queueing.
    exists: Option<bool>,
}

struct DownloadPlan {
    item: String,
    name: String,
    key: String,
    path: std::path::PathBuf,
    size: Option<i64>,
}

/// Expands one upload item: a file selection, a folder selection (every file
/// inside it), or `<folder selection><SEP><relative path>` for one file.
async fn plan_upload(
    provider: &S3StorageProvider,
    bucket: &str,
    prefix: &str,
    item: &str,
    selections: &SelectionRegistry,
) -> Result<Vec<UploadPlan>, AppError> {
    let (selection_id, only) = match item.split_once(ITEM_SEPARATOR) {
        Some((id, relative)) => (id, Some(relative)),
        None => (item, None),
    };
    let selection = selections.get_any(
        selection_id,
        &[SelectionKind::File, SelectionKind::UploadDirectory],
    )?;
    let name = selection
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    if selection.kind == SelectionKind::File {
        let size = std::fs::metadata(&selection.path).map(|m| m.len()).ok();
        return Ok(vec![UploadPlan {
            item: item.to_owned(),
            key: format!("{prefix}{name}"),
            name,
            path: selection.path,
            size: size.and_then(|s| i64::try_from(s).ok()),
            exists: None,
        }]);
    }

    let base = format!("{prefix}{name}/");
    let dir = selection.path.clone();
    let mut files =
        tauri::async_runtime::spawn_blocking(move || collect_files(&dir, MAX_FOLDER_FILES))
            .await
            .map_err(|_| AppError::internal("Reading the folder was interrupted."))??;
    if let Some(relative) = only {
        files.retain(|(_, r)| r == relative);
    }
    let existing = existing_keys(provider, bucket, &base).await?;

    Ok(files
        .into_iter()
        .map(|(path, relative)| {
            let key = format!("{base}{relative}");
            let size = std::fs::metadata(&path).map(|m| m.len()).ok();
            UploadPlan {
                item: format!("{selection_id}{ITEM_SEPARATOR}{relative}"),
                name: format!("{name}/{relative}"),
                exists: existing.as_ref().map(|set| set.contains(&key)),
                key,
                path,
                size: size.and_then(|s| i64::try_from(s).ok()),
            }
        })
        .collect())
}

/// Expands one download item: an object key, a folder key ending in `/`
/// (every object under it), or `<folder key><SEP><object key>`.
async fn plan_download(
    provider: &S3StorageProvider,
    bucket: &str,
    dir: &std::path::Path,
    item: &str,
) -> Result<Vec<DownloadPlan>, AppError> {
    let (folder, only) = match item.split_once(ITEM_SEPARATOR) {
        Some((folder, key)) => (Some(folder), Some(key)),
        None if item.ends_with('/') => (Some(item), None),
        None => (None, None),
    };

    let Some(folder) = folder else {
        let base = item.rsplit('/').next().unwrap_or(item);
        let path = safe_local_file_name(base)
            .and_then(|name| join_in_dir(dir, &name))
            .ok_or_else(unusable_name)?;
        return Ok(vec![DownloadPlan {
            item: item.to_owned(),
            name: base.to_owned(),
            key: item.to_owned(),
            path,
            size: None,
        }]);
    };

    // Paths are relative to the folder's parent, so the folder itself is
    // recreated inside the destination.
    let trimmed = folder.trim_end_matches('/');
    let parent = trimmed.rfind('/').map_or("", |i| &trimmed[..=i]);

    let objects = match only {
        Some(key) => vec![(key.to_owned(), None)],
        None => list_folder(provider, bucket, folder).await?,
    };
    objects
        .into_iter()
        .filter(|(key, _)| !key.ends_with('/'))
        .map(|(key, size)| {
            let relative = key.strip_prefix(parent).unwrap_or(&key).to_owned();
            let path = join_relative(dir, &relative).ok_or_else(unusable_name)?;
            if !stays_within(dir, &path) {
                return Err(AppError::new(
                    ErrorCode::LocalFileError,
                    format!(
                        "\"{relative}\" would be saved outside the chosen folder because of a \
                         shortcut (symbolic link) inside it. Choose another folder."
                    ),
                ));
            }
            Ok(DownloadPlan {
                item: format!("{folder}{ITEM_SEPARATOR}{key}"),
                name: relative,
                key,
                path,
                size,
            })
        })
        .collect()
}

/// Every object under `prefix`, refusing folders larger than the limit.
async fn list_folder(
    provider: &S3StorageProvider,
    bucket: &str,
    prefix: &str,
) -> Result<Vec<(String, Option<i64>)>, AppError> {
    let mut objects = Vec::new();
    let mut token: Option<String> = None;
    loop {
        let page = provider
            .list_objects(ListQuery {
                bucket,
                prefix,
                delimiter: None,
                continuation_token: token.as_deref(),
                page_size: Some(1000),
            })
            .await?;
        objects.extend(page.objects.into_iter().map(|o| (o.key, o.size)));
        if objects.len() > MAX_FOLDER_FILES {
            return Err(AppError::new(
                ErrorCode::OperationUnsupported,
                format!(
                    "This folder contains more than {MAX_FOLDER_FILES} objects, which is more than Bucklane can download at once."
                ),
            ));
        }
        if !page.is_truncated {
            return Ok(objects);
        }
        token = page.next_continuation_token;
    }
}

/// Keys already under `prefix`, or `None` if there are too many to gather
/// (callers then check each file individually).
async fn existing_keys(
    provider: &S3StorageProvider,
    bucket: &str,
    prefix: &str,
) -> Result<Option<std::collections::HashSet<String>>, AppError> {
    let mut keys = std::collections::HashSet::new();
    let mut token: Option<String> = None;
    loop {
        let page = provider
            .list_objects(ListQuery {
                bucket,
                prefix,
                delimiter: None,
                continuation_token: token.as_deref(),
                page_size: Some(1000),
            })
            .await?;
        keys.extend(page.objects.into_iter().map(|o| o.key));
        if keys.len() > MAX_EXISTING_KEYS {
            return Ok(None);
        }
        if !page.is_truncated {
            return Ok(Some(keys));
        }
        token = page.next_continuation_token;
    }
}

fn unusable_name() -> AppError {
    AppError::new(
        ErrorCode::LocalFileError,
        "This object's name cannot be used as a file name.",
    )
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
                resume_etag: None,
                resume_offset: 0,
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
        upload::run(
            &f.client,
            &f.repo,
            &mut up,
            &progress,
            upload::UploadTuning::default(),
        )
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

    /// Uploads `data` and returns the object's ETag.
    async fn put_object(f: &Fixture, key: &str, data: Vec<u8>) -> String {
        f.client
            .put_object()
            .bucket(&f.bucket)
            .key(key)
            .body(aws_sdk_s3::primitives::ByteStream::from(data))
            .send()
            .await
            .unwrap()
            .e_tag()
            .unwrap()
            .to_owned()
    }

    #[tokio::test]
    async fn interrupted_download_resumes_from_its_checkpoint() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        let data: Vec<u8> = (0..3 * 1024 * 1024u32).map(|i| (i % 251) as u8).collect();
        let etag = put_object(&f, "video.bin", data.clone()).await;

        let target = f.dir.join("video.bin");
        let mut down = f
            .transfer(TransferDirection::Download, "video.bin", &target, false)
            .await;
        // As after a power loss: valid bytes up to the checkpoint, then
        // garbage that never made it safely to disk.
        let checkpoint = 1024 * 1024;
        let mut partial = data[..checkpoint].to_vec();
        partial.extend(std::iter::repeat_n(0xEE, 4096));
        std::fs::write(download::temp_path(&down).unwrap(), &partial).unwrap();
        down.resume_etag = Some(etag);
        down.resume_offset = checkpoint as i64;

        let progress = Progress::default();
        download::run(&f.client, &f.repo, &mut down, &progress)
            .await
            .unwrap();

        assert!(
            std::fs::read(&target).unwrap() == data,
            "resumed bytes differ"
        );
        assert_eq!(progress.total(), data.len() as i64);
        assert_eq!((down.resume_etag.as_deref(), down.resume_offset), (None, 0));
        assert!(download::temp_path(&down).is_some_and(|p| !p.exists()));
    }

    #[tokio::test]
    async fn changed_object_restarts_instead_of_resuming() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        let data = b"the new version of this file".to_vec();
        put_object(&f, "notes.txt", data.clone()).await;

        let target = f.dir.join("notes.txt");
        let mut down = f
            .transfer(TransferDirection::Download, "notes.txt", &target, false)
            .await;
        // Partial data from an older version of the object.
        std::fs::write(download::temp_path(&down).unwrap(), b"OLD VERSION").unwrap();
        down.resume_etag = Some("\"stale-etag\"".to_owned());
        down.resume_offset = 11;

        download::run(&f.client, &f.repo, &mut down, &Progress::default())
            .await
            .unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), data);
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
        upload::run(
            &f.client,
            &f.repo,
            &mut up,
            &progress,
            upload::UploadTuning::default(),
        )
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
    async fn folder_upload_plan_keeps_structure_and_detects_existing() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        let local = f.dir.join("reports");
        std::fs::create_dir_all(local.join("2026")).unwrap();
        std::fs::write(local.join("summary.txt"), b"s").unwrap();
        std::fs::write(local.join("2026/q3.pdf"), b"q").unwrap();
        std::fs::write(local.join(".DS_Store"), b"x").unwrap();
        // Already uploaded earlier.
        f.client
            .put_object()
            .bucket(&f.bucket)
            .key("docs/reports/summary.txt")
            .body(aws_sdk_s3::primitives::ByteStream::from_static(b"old"))
            .send()
            .await
            .unwrap();

        let registry = SelectionRegistry::default();
        let id = registry
            .register_upload_items([local])
            .pop()
            .unwrap()
            .selection_id;
        let provider = S3StorageProvider::new(f.client.clone());
        let plans = plan_upload(&provider, &f.bucket, "docs/", &id, &registry)
            .await
            .unwrap();

        let summary: Vec<_> = plans.iter().map(|p| (p.key.as_str(), p.exists)).collect();
        assert_eq!(
            summary,
            [
                ("docs/reports/2026/q3.pdf", Some(false)),
                ("docs/reports/summary.txt", Some(true))
            ]
        );

        // A conflict resend targets only that one file.
        let resend = plans[1].item.clone();
        let single = plan_upload(&provider, &f.bucket, "docs/", &resend, &registry)
            .await
            .unwrap();
        assert_eq!(single.len(), 1);
        assert_eq!(single[0].key, "docs/reports/summary.txt");
    }

    #[tokio::test]
    async fn folder_download_plan_recreates_structure_inside_destination() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        for key in [
            "media/brand/",
            "media/brand/logo.svg",
            "media/brand/print/card.pdf",
            "media/other.txt",
        ] {
            f.client
                .put_object()
                .bucket(&f.bucket)
                .key(key)
                .body(aws_sdk_s3::primitives::ByteStream::from_static(b"x"))
                .send()
                .await
                .unwrap();
        }
        let provider = S3StorageProvider::new(f.client.clone());
        let plans = plan_download(&provider, &f.bucket, &f.dir, "media/brand/")
            .await
            .unwrap();

        let mut paths: Vec<_> = plans
            .iter()
            .map(|p| {
                p.path
                    .strip_prefix(&f.dir)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        paths.sort();
        assert_eq!(
            paths,
            ["brand/logo.svg", "brand/print/card.pdf"],
            "marker skipped, sibling excluded"
        );
        assert!(plans.iter().all(|p| p.path.starts_with(&f.dir)));
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
        upload::run(
            &f.client,
            &f.repo,
            &mut first,
            &Progress::default(),
            upload::UploadTuning::default(),
        )
        .await
        .unwrap();

        std::fs::write(&source, b"second").unwrap();
        let mut second = f
            .transfer(TransferDirection::Upload, "a.txt", &source, false)
            .await;
        let err = upload::run(
            &f.client,
            &f.repo,
            &mut second,
            &Progress::default(),
            upload::UploadTuning::default(),
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::TransferFailed);
    }
}
