use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use chrono::{SecondsFormat, Utc};
use futures_util::StreamExt;

use crate::domain::bucket::validate_bucket_name;
use crate::domain::object::{
    ConflictPolicy, CopyObjectInput, CreateFolderInput, CreateShareLinkInput, DeleteFailure,
    DeleteObjectsInput, DeleteObjectsResult, ExistingObjectPolicy, ListObjectsRequest,
    ListObjectsResponse, ObjectMetadata, ObjectOperationOutcome, ObjectPreview, PrefixDeletePlan,
    PrefixRelocateInput, PrefixRelocatePlan, PrefixRelocateResult, PreviewKind, RelocateMode,
    ShareLink,
};
use crate::errors::{AppError, ErrorCode};
use crate::providers::s3_provider::S3StorageProvider;
use crate::providers::storage_provider::{ListQuery, StorageProvider};
use crate::services::connection_service::ConnectionService;
use crate::services::delete_plans::{DeletePlanRegistry, PLAN_TTL};
use crate::services::relocate_plans::{RelocateItem, RelocatePlanRegistry};

const DEFAULT_DELIMITER: &str = "/";
/// S3 keys are at most 1024 bytes.
const MAX_KEY_LEN: usize = 1024;
/// Largest object a single CopyObject request may copy; larger objects need
/// multipart copy, which the MVP does not perform (TSD §9.6).
const MAX_SINGLE_COPY_BYTES: i64 = 5 * 1024 * 1024 * 1024;
/// Largest folder Bucklane copies or moves at once. Each object costs a few
/// requests, so this is lower than the delete limit.
const MAX_PREFIX_RELOCATE_OBJECTS: usize = 10_000;
/// Objects copied in parallel during a folder copy/move.
const RELOCATE_CONCURRENCY: usize = 8;
const RELOCATE_PROGRESS_INTERVAL: Duration = Duration::from_millis(150);
const DELETE_BATCH: usize = 1000;
const MAX_PREFIX_DELETE_OBJECTS: usize = 100_000;
const SAMPLE_KEYS: usize = 10;
/// Preview limits: images and PDFs load whole, text loads only its beginning.
const MAX_IMAGE_PREVIEW_BYTES: u64 = 15 * 1024 * 1024;
const MAX_PDF_PREVIEW_BYTES: u64 = 25 * 1024 * 1024;
/// Image types the webview may render. Anything else is never handed to the
/// frontend as an image, whatever the object's stored Content-Type claims.
const PREVIEW_IMAGE_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/avif",
    "image/bmp",
    "image/svg+xml",
    "image/x-icon",
    "image/vnd.microsoft.icon",
];
const TEXT_PREVIEW_BYTES: u64 = 512 * 1024;

/// Presigned URL lifetime bounds; SigV4 signatures are valid for at most 7 days.
const MIN_SHARE_SECONDS: u32 = 60;
const MAX_SHARE_SECONDS: u32 = 7 * 24 * 60 * 60;

/// Object operations for the explorer (EXP-*, FIL-003..006).
pub struct ObjectService<'a> {
    connections: ConnectionService<'a>,
    delete_plans: &'a DeletePlanRegistry,
    relocate_plans: &'a RelocatePlanRegistry,
}

impl<'a> ObjectService<'a> {
    pub fn new(
        connections: ConnectionService<'a>,
        delete_plans: &'a DeletePlanRegistry,
        relocate_plans: &'a RelocatePlanRegistry,
    ) -> Self {
        Self {
            connections,
            delete_plans,
            relocate_plans,
        }
    }

    async fn provider(
        &self,
        connection_id: &str,
        bucket: &str,
    ) -> Result<S3StorageProvider, AppError> {
        let client = self
            .connections
            .client_for_bucket(connection_id, bucket)
            .await?;
        Ok(S3StorageProvider::new(client))
    }

    /// One page of a folder-style listing. Never lists a whole bucket at once.
    pub async fn list_objects(
        &self,
        request: ListObjectsRequest,
    ) -> Result<ListObjectsResponse, AppError> {
        let bucket = validate_bucket_name(&request.bucket)?;
        let prefix = request.prefix.as_deref().unwrap_or("");
        check_len(prefix, "prefix")?;
        let delimiter = request.delimiter.as_deref().unwrap_or(DEFAULT_DELIMITER);

        self.provider(&request.connection_id, bucket)
            .await?
            .list_objects(ListQuery {
                bucket,
                prefix,
                delimiter: Some(delimiter),
                continuation_token: request.continuation_token.as_deref(),
                page_size: request.page_size,
            })
            .await
    }

    pub async fn head_object(
        &self,
        connection_id: &str,
        bucket: &str,
        key: &str,
    ) -> Result<ObjectMetadata, AppError> {
        let bucket = validate_bucket_name(bucket)?;
        validate_key(key)?;
        self.provider(connection_id, bucket)
            .await?
            .head_object(bucket, key)
            .await
    }

    /// Loads an object for in-app preview within the size limits above.
    pub async fn preview_object(
        &self,
        connection_id: &str,
        bucket: &str,
        key: &str,
    ) -> Result<ObjectPreview, AppError> {
        use base64::Engine;

        let bucket = validate_bucket_name(bucket)?;
        validate_key(key)?;
        let provider = self.provider(connection_id, bucket).await?;
        let metadata = provider.head_object(bucket, key).await?;
        let Some(kind) = preview_kind(key, metadata.content_type.as_deref()) else {
            return Err(AppError::new(
                ErrorCode::OperationUnsupported,
                "This type of file can't be previewed. Download it to open it.",
            ));
        };

        let limit = match kind {
            PreviewKind::Image => MAX_IMAGE_PREVIEW_BYTES,
            PreviewKind::Pdf => MAX_PDF_PREVIEW_BYTES,
            PreviewKind::Text => TEXT_PREVIEW_BYTES,
        };
        let too_large = || {
            AppError::new(
                ErrorCode::OperationUnsupported,
                format!(
                    "This file is too large to preview (limit {} MB). Download it to open it.",
                    limit / (1024 * 1024)
                ),
            )
        };
        let reported_size = metadata.size.map(|s| s.max(0) as u64);
        if kind != PreviewKind::Text && reported_size.is_some_and(|s| s > limit) {
            return Err(too_large());
        }

        // The cap also holds when HEAD reports no size or the object grows
        // between HEAD and GET.
        let (bytes, more) = provider.get_bytes(bucket, key, limit).await?;
        if more && kind != PreviewKind::Text {
            return Err(too_large());
        }
        let truncated = kind == PreviewKind::Text
            && (more || reported_size.is_some_and(|s| s > bytes.len() as u64));
        let content_type = preview_content_type(kind, key, metadata.content_type.as_deref());

        Ok(ObjectPreview {
            kind,
            content_type,
            data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            truncated,
            size: metadata.size,
        })
    }

    /// Presigned download link for one object. The object's existence is
    /// checked first so the user never shares a link that cannot work.
    pub async fn create_share_link(
        &self,
        input: CreateShareLinkInput,
    ) -> Result<ShareLink, AppError> {
        let bucket = validate_bucket_name(&input.bucket)?;
        validate_key(&input.key)?;
        if input.key.ends_with('/') {
            return Err(AppError::new(
                ErrorCode::OperationUnsupported,
                "Links can only be created for files, not folders.",
            ));
        }
        let seconds = validate_share_duration(input.expires_in_seconds)?;

        let provider = self.provider(&input.connection_id, bucket).await?;
        provider.head_object(bucket, &input.key).await?;
        let expires_in = std::time::Duration::from_secs(u64::from(seconds));
        let url = provider.presign_get(bucket, &input.key, expires_in).await?;
        let expires_at = Utc::now() + chrono::Duration::seconds(i64::from(seconds));

        // The URL itself is a bearer credential and is deliberately not logged.
        tracing::info!(connection_id = %input.connection_id, seconds, "share link created");
        Ok(ShareLink {
            url,
            expires_at: expires_at.to_rfc3339_opts(SecondsFormat::Millis, true),
        })
    }

    /// Creates a zero-byte `name/` marker so an empty folder is visible
    /// (EXP-003). The marker is not a real directory.
    pub async fn create_folder(&self, input: CreateFolderInput) -> Result<String, AppError> {
        let bucket = validate_bucket_name(&input.bucket)?;
        let name = validate_folder_name(&input.name)?;
        if !input.prefix.is_empty() && !input.prefix.ends_with('/') {
            return Err(invalid("The parent folder is not valid."));
        }
        let key = format!("{}{name}/", input.prefix);
        validate_key(&key)?;

        let provider = self.provider(&input.connection_id, bucket).await?;
        // A folder exists if its marker exists or anything is stored under
        // it. Listings hide the listed folder's own marker, so check the
        // marker directly first.
        match provider.head_object(bucket, &key).await {
            Ok(_) => return Err(invalid("A folder with this name already exists.")),
            Err(err) if err.code == ErrorCode::ObjectNotFound => {}
            Err(err) => return Err(err),
        }
        let existing = provider
            .list_objects(ListQuery {
                bucket,
                prefix: &key,
                delimiter: Some(DEFAULT_DELIMITER),
                continuation_token: None,
                page_size: Some(1),
            })
            .await?;
        if !existing.objects.is_empty() || existing.is_truncated {
            return Err(invalid("A folder with this name already exists."));
        }

        provider.put_empty_object(bucket, &key).await?;
        tracing::info!(connection_id = %input.connection_id, "folder created");
        Ok(key)
    }

    pub async fn copy_object(
        &self,
        input: CopyObjectInput,
    ) -> Result<ObjectOperationOutcome, AppError> {
        let (provider, source) = self.prepare_copy(&input).await?;
        if let Some(conflict) = self.check_destination(&provider, &input).await? {
            return Ok(conflict);
        }
        copy_and_verify(&provider, &input, &source).await?;
        tracing::info!(connection_id = %input.connection_id, "object copied");
        Ok(ObjectOperationOutcome::Completed)
    }

    /// Safe move (TSD §9.6): copy with `If-Match` on the source ETag, verify
    /// the destination, re-check the source is unchanged, and only then
    /// delete it. Any doubt keeps the source.
    pub async fn move_object(
        &self,
        input: CopyObjectInput,
    ) -> Result<ObjectOperationOutcome, AppError> {
        let (provider, source) = self.prepare_copy(&input).await?;
        if let Some(conflict) = self.check_destination(&provider, &input).await? {
            return Ok(conflict);
        }
        copy_and_verify(&provider, &input, &source).await?;

        let bucket = input.bucket.trim();
        match provider.head_object(bucket, &input.source_key).await {
            Ok(current) if current.etag == source.etag => {}
            Ok(_) => {
                return Ok(source_kept(
                    "The original changed while it was being moved, so it was kept.",
                ));
            }
            Err(err) => return Ok(source_kept(&err.message)),
        }

        if let Err(err) = provider.delete_object(bucket, &input.source_key).await {
            return Ok(source_kept(&format!(
                "The copy succeeded, but the original could not be deleted: {}",
                err.message
            )));
        }
        tracing::info!(connection_id = %input.connection_id, "object moved");
        Ok(ObjectOperationOutcome::Completed)
    }

    pub async fn delete_objects(
        &self,
        input: DeleteObjectsInput,
    ) -> Result<DeleteObjectsResult, AppError> {
        let bucket = validate_bucket_name(&input.bucket)?;
        if input.keys.is_empty() {
            return Ok(DeleteObjectsResult::default());
        }
        for key in &input.keys {
            validate_key(key)?;
        }
        let provider = self.provider(&input.connection_id, bucket).await?;
        let result = delete_in_batches(&provider, bucket, &input.keys).await;
        tracing::info!(
            connection_id = %input.connection_id,
            deleted = result.deleted.len(),
            failed = result.failed.len(),
            "objects deleted"
        );
        Ok(result)
    }

    /// Lists every key under `prefix` and stores it as a single-use plan; the
    /// user confirms against the returned counts.
    pub async fn prepare_prefix_delete(
        &self,
        connection_id: &str,
        bucket: &str,
        prefix: &str,
    ) -> Result<PrefixDeletePlan, AppError> {
        let bucket = validate_bucket_name(bucket)?;
        if prefix.is_empty() {
            return Err(invalid(
                "Deleting an entire bucket's contents is not allowed.",
            ));
        }
        if !prefix.ends_with('/') {
            return Err(invalid("Only folders can be deleted this way."));
        }
        validate_key(prefix)?;

        let provider = self.provider(connection_id, bucket).await?;
        let mut keys = Vec::new();
        let mut total_bytes = 0i64;
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
            for object in page.objects {
                total_bytes += object.size.unwrap_or(0);
                keys.push(object.key);
            }
            if keys.len() > MAX_PREFIX_DELETE_OBJECTS {
                return Err(AppError::new(
                    ErrorCode::OperationUnsupported,
                    format!(
                        "This folder contains more than {MAX_PREFIX_DELETE_OBJECTS} objects, which is more than Bucklane can delete at once."
                    ),
                ));
            }
            if !page.is_truncated {
                break;
            }
            token = page.next_continuation_token;
        }
        // Listings hide the marker of the listed folder itself; include it
        // only if it really exists so the confirmed count stays exact.
        if !keys.iter().any(|k| k == prefix) {
            match provider.head_object(bucket, prefix).await {
                Ok(marker) => {
                    total_bytes += marker.size.unwrap_or(0);
                    keys.push(prefix.to_owned());
                }
                Err(err) if err.code == ErrorCode::ObjectNotFound => {}
                Err(err) => return Err(err),
            }
        }
        if keys.is_empty() {
            return Err(invalid("This folder is already empty."));
        }

        let sample_keys = keys.iter().take(SAMPLE_KEYS).cloned().collect();
        let object_count = u32::try_from(keys.len()).unwrap_or(u32::MAX);
        let expires_at = Utc::now() + PLAN_TTL;
        let plan_id = self.delete_plans.insert(connection_id, bucket, keys);

        Ok(PrefixDeletePlan {
            plan_id,
            bucket: bucket.to_owned(),
            prefix: prefix.to_owned(),
            object_count,
            total_bytes,
            sample_keys,
            expires_at: expires_at.to_rfc3339_opts(SecondsFormat::Millis, true),
        })
    }

    /// Deletes exactly the keys captured in the plan; objects added after the
    /// plan was created are left alone.
    pub async fn execute_prefix_delete(
        &self,
        plan_id: &str,
    ) -> Result<DeleteObjectsResult, AppError> {
        let plan = self.delete_plans.take(plan_id)?;
        let provider = self.provider(&plan.connection_id, &plan.bucket).await?;
        let result = delete_in_batches(&provider, &plan.bucket, &plan.keys).await;
        tracing::info!(
            connection_id = %plan.connection_id,
            deleted = result.deleted.len(),
            failed = result.failed.len(),
            "folder deleted"
        );
        Ok(result)
    }

    /// Snapshots a folder copy/move: every object under the source, where it
    /// will go, and which destinations already exist. Nothing is changed.
    pub async fn prepare_prefix_relocate(
        &self,
        input: PrefixRelocateInput,
    ) -> Result<PrefixRelocatePlan, AppError> {
        let bucket = validate_bucket_name(&input.bucket)?;
        let (source, destination) = (&input.source_prefix, &input.destination_prefix);
        validate_relocate_prefixes(source, destination)?;

        let provider = self.provider(&input.connection_id, bucket).await?;
        let objects = list_everything(&provider, bucket, source, MAX_PREFIX_RELOCATE_OBJECTS)
            .await?
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::OperationUnsupported,
                    format!(
                        "This folder contains more than {MAX_PREFIX_RELOCATE_OBJECTS} objects, which is more than Bucklane can copy or move at once."
                    ),
                )
            })?;
        if objects.is_empty() {
            return Err(invalid(
                "This folder is empty, so there is nothing to copy or move.",
            ));
        }
        if let Some(big) = objects.iter().find(|o| o.size > MAX_SINGLE_COPY_BYTES) {
            return Err(AppError::new(
                ErrorCode::OperationUnsupported,
                "This folder contains an object larger than 5 GiB, which Bucklane cannot copy or move yet.",
            )
            .with_details(big.key.clone()));
        }

        // Existing destination keys, to report conflicts up front.
        let existing: HashSet<String> =
            list_everything(&provider, bucket, destination, MAX_PREFIX_DELETE_OBJECTS)
                .await?
                .unwrap_or_default()
                .into_iter()
                .map(|o| o.key)
                .collect();

        let mut items = Vec::with_capacity(objects.len());
        for object in objects {
            let relative = object
                .key
                .strip_prefix(source.as_str())
                .unwrap_or(&object.key);
            let target = format!("{destination}{relative}");
            validate_key(&target)?;
            items.push(RelocateItem {
                conflict: existing.contains(&target),
                source: object.key,
                destination: target,
                size: object.size,
                etag: object.etag,
            });
        }

        let conflicts: Vec<&RelocateItem> = items.iter().filter(|i| i.conflict).collect();
        let conflict_count = u32::try_from(conflicts.len()).unwrap_or(u32::MAX);
        let conflict_samples = conflicts
            .iter()
            .take(SAMPLE_KEYS)
            .map(|i| i.destination.clone())
            .collect();
        let object_count = u32::try_from(items.len()).unwrap_or(u32::MAX);
        let total_bytes = items.iter().map(|i| i.size).sum();
        let expires_at = Utc::now() + PLAN_TTL;
        let plan_id = self
            .relocate_plans
            .insert(&input.connection_id, bucket, input.mode, items);

        Ok(PrefixRelocatePlan {
            plan_id,
            mode: input.mode,
            source_prefix: source.clone(),
            destination_prefix: destination.clone(),
            object_count,
            total_bytes,
            conflict_count,
            conflict_samples,
            expires_at: expires_at.to_rfc3339_opts(SecondsFormat::Millis, true),
        })
    }

    /// Runs a confirmed plan. Each object is copied with `If-Match` on its
    /// snapshotted ETag and verified by size; for a move, an original is
    /// deleted only if it is still unchanged afterwards (same rules as a
    /// single-object move). `on_progress(done, total)` is throttled.
    pub async fn execute_prefix_relocate(
        &self,
        plan_id: &str,
        existing: ExistingObjectPolicy,
        on_progress: impl Fn(u32, u32),
    ) -> Result<PrefixRelocateResult, AppError> {
        let (plan, cancel) = self.relocate_plans.start(plan_id)?;
        let result = self
            .run_relocate(
                &plan.connection_id,
                &plan.bucket,
                plan.mode,
                plan.items,
                existing,
                &cancel,
                on_progress,
            )
            .await;
        self.relocate_plans.finish(plan_id);
        let result = result?;
        tracing::info!(
            connection_id = %plan.connection_id,
            mode = ?plan.mode,
            completed = result.completed,
            skipped = result.skipped,
            canceled = result.canceled,
            failed = result.failed.len(),
            sources_kept = result.sources_kept.len(),
            "folder relocated"
        );
        Ok(result)
    }

    pub fn cancel_prefix_relocate(&self, plan_id: &str) {
        self.relocate_plans.cancel(plan_id);
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_relocate(
        &self,
        connection_id: &str,
        bucket: &str,
        mode: RelocateMode,
        items: Vec<RelocateItem>,
        existing: ExistingObjectPolicy,
        cancel: &AtomicBool,
        on_progress: impl Fn(u32, u32),
    ) -> Result<PrefixRelocateResult, AppError> {
        let provider = self.provider(connection_id, bucket).await?;
        let total = u32::try_from(items.len()).unwrap_or(u32::MAX);
        let mut result = PrefixRelocateResult::default();
        let mut to_delete = Vec::new();
        let mut done = 0u32;
        let mut last_report: Option<Instant> = None;

        let mut outcomes = futures_util::stream::iter(items)
            .map(|item| {
                let provider = &provider;
                async move {
                    if cancel.load(Ordering::SeqCst) {
                        return (item, ItemOutcome::Canceled);
                    }
                    if item.conflict && existing == ExistingObjectPolicy::Skip {
                        return (item, ItemOutcome::Skipped);
                    }
                    let outcome = relocate_item(provider, bucket, mode, &item).await;
                    (item, outcome)
                }
            })
            .buffer_unordered(RELOCATE_CONCURRENCY);

        while let Some((item, outcome)) = outcomes.next().await {
            match outcome {
                ItemOutcome::Done => result.completed += 1,
                ItemOutcome::DeleteSource => {
                    result.completed += 1;
                    to_delete.push(item.source);
                }
                ItemOutcome::Skipped => result.skipped += 1,
                ItemOutcome::Canceled => result.canceled += 1,
                ItemOutcome::Failed(error) => result.failed.push(DeleteFailure {
                    key: item.source,
                    error,
                }),
                ItemOutcome::SourceKept(error) => {
                    result.completed += 1;
                    result.sources_kept.push(DeleteFailure {
                        key: item.source,
                        error,
                    });
                }
            }
            done += 1;
            if done == total
                || last_report.is_none_or(|t| t.elapsed() >= RELOCATE_PROGRESS_INTERVAL)
            {
                last_report = Some(Instant::now());
                on_progress(done, total);
            }
        }

        if !to_delete.is_empty() {
            let deleted = delete_in_batches(&provider, bucket, &to_delete).await;
            result.sources_kept.extend(deleted.failed);
        }
        Ok(result)
    }

    async fn prepare_copy(
        &self,
        input: &CopyObjectInput,
    ) -> Result<(S3StorageProvider, ObjectMetadata), AppError> {
        let bucket = validate_bucket_name(&input.bucket)?;
        validate_key(&input.source_key)?;
        validate_key(&input.destination_key)?;
        if input.source_key == input.destination_key {
            return Err(invalid("The destination is the same as the source."));
        }
        if input.source_key.ends_with('/') || input.destination_key.ends_with('/') {
            return Err(AppError::new(
                ErrorCode::OperationUnsupported,
                "Copying, moving, or renaming folders is not supported yet.",
            ));
        }

        let provider = self.provider(&input.connection_id, bucket).await?;
        let source = provider.head_object(bucket, &input.source_key).await?;
        if source.size.unwrap_or(0) > MAX_SINGLE_COPY_BYTES {
            return Err(AppError::new(
                ErrorCode::OperationUnsupported,
                "Objects larger than 5 GiB cannot be copied, moved, or renamed yet.",
            ));
        }
        Ok((provider, source))
    }

    async fn check_destination(
        &self,
        provider: &S3StorageProvider,
        input: &CopyObjectInput,
    ) -> Result<Option<ObjectOperationOutcome>, AppError> {
        match provider
            .head_object(input.bucket.trim(), &input.destination_key)
            .await
        {
            Ok(existing) if input.conflict_policy == ConflictPolicy::Ask => {
                Ok(Some(ObjectOperationOutcome::Conflict {
                    existing_size: existing.size,
                    existing_last_modified: existing.last_modified,
                }))
            }
            Ok(_) => Ok(None),
            Err(err) if err.code == ErrorCode::ObjectNotFound => Ok(None),
            Err(err) => Err(err),
        }
    }
}

async fn copy_and_verify(
    provider: &S3StorageProvider,
    input: &CopyObjectInput,
    source: &ObjectMetadata,
) -> Result<(), AppError> {
    let bucket = input.bucket.trim();
    provider
        .copy_object(
            bucket,
            &input.source_key,
            &input.destination_key,
            source.etag.as_deref(),
        )
        .await?;

    // ETags of copies are not guaranteed to match, so verify by size.
    let copied = provider.head_object(bucket, &input.destination_key).await?;
    if copied.size != source.size {
        return Err(AppError::new(
            ErrorCode::TransferFailed,
            "The copy could not be verified: its size differs from the original.",
        ));
    }
    Ok(())
}

enum ItemOutcome {
    Done,
    /// Move: copied and verified; the unchanged original can be deleted.
    DeleteSource,
    Skipped,
    Canceled,
    Failed(AppError),
    SourceKept(AppError),
}

/// Copies one object of a folder copy/move and decides what happens to the
/// original. Any doubt keeps the original.
async fn relocate_item(
    provider: &S3StorageProvider,
    bucket: &str,
    mode: RelocateMode,
    item: &RelocateItem,
) -> ItemOutcome {
    if let Err(err) = provider
        .copy_object(
            bucket,
            &item.source,
            &item.destination,
            item.etag.as_deref(),
        )
        .await
    {
        return ItemOutcome::Failed(err);
    }
    match provider.head_object(bucket, &item.destination).await {
        Ok(copied) if copied.size.unwrap_or(0) == item.size => {}
        Ok(_) => {
            return ItemOutcome::Failed(AppError::new(
                ErrorCode::TransferFailed,
                "The copy could not be verified: its size differs from the original.",
            ));
        }
        Err(err) => return ItemOutcome::Failed(err),
    }
    if mode == RelocateMode::Copy {
        return ItemOutcome::Done;
    }
    match provider.head_object(bucket, &item.source).await {
        Ok(current) if current.etag == item.etag => ItemOutcome::DeleteSource,
        Ok(_) => ItemOutcome::SourceKept(AppError::new(
            ErrorCode::TransferFailed,
            "The original changed while it was being moved, so it was kept.",
        )),
        Err(err) => ItemOutcome::SourceKept(err),
    }
}

struct ListedObject {
    key: String,
    size: i64,
    etag: Option<String>,
}

/// Every object under `prefix`, including the folder's own marker object if
/// it exists. `None` when there are more than `max`.
async fn list_everything(
    provider: &S3StorageProvider,
    bucket: &str,
    prefix: &str,
    max: usize,
) -> Result<Option<Vec<ListedObject>>, AppError> {
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
        objects.extend(page.objects.into_iter().map(|o| ListedObject {
            key: o.key,
            size: o.size.unwrap_or(0),
            etag: o.etag,
        }));
        if objects.len() > max {
            return Ok(None);
        }
        if !page.is_truncated {
            break;
        }
        token = page.next_continuation_token;
    }
    // Listings hide the marker of the listed folder itself.
    if !objects.iter().any(|o| o.key == prefix) {
        match provider.head_object(bucket, prefix).await {
            Ok(marker) => objects.push(ListedObject {
                key: prefix.to_owned(),
                size: marker.size.unwrap_or(0),
                etag: marker.etag,
            }),
            Err(err) if err.code == ErrorCode::ObjectNotFound => {}
            Err(err) => return Err(err),
        }
    }
    Ok(Some(objects))
}

fn validate_relocate_prefixes(source: &str, destination: &str) -> Result<(), AppError> {
    if source.is_empty() || !source.ends_with('/') || !destination.ends_with('/') {
        return Err(invalid("Only folders can be copied or moved this way."));
    }
    if destination.trim_matches('/').is_empty() {
        return Err(invalid("Enter a folder name for the destination."));
    }
    validate_key(source)?;
    validate_key(destination)?;
    if source == destination {
        return Err(invalid("The destination is the same as the source."));
    }
    if destination.starts_with(source) {
        return Err(invalid("A folder cannot be copied or moved into itself."));
    }
    Ok(())
}

/// Uses DeleteObjects in batches of 1000, falling back to one request per key
/// for providers that do not implement the batch API.
async fn delete_in_batches(
    provider: &S3StorageProvider,
    bucket: &str,
    keys: &[String],
) -> DeleteObjectsResult {
    let mut result = DeleteObjectsResult::default();
    for batch in keys.chunks(DELETE_BATCH) {
        match provider.delete_objects(bucket, batch).await {
            Ok(batch_result) => {
                result.deleted.extend(batch_result.deleted);
                result.failed.extend(batch_result.failed);
            }
            Err(err)
                if matches!(
                    err.code,
                    ErrorCode::OperationUnsupported | ErrorCode::InternalError
                ) =>
            {
                for key in batch {
                    match provider.delete_object(bucket, key).await {
                        Ok(()) => result.deleted.push(key.clone()),
                        Err(error) => result.failed.push(crate::domain::object::DeleteFailure {
                            key: key.clone(),
                            error,
                        }),
                    }
                }
            }
            Err(err) => {
                result.failed.extend(batch.iter().map(|key| {
                    crate::domain::object::DeleteFailure {
                        key: key.clone(),
                        error: err.clone(),
                    }
                }));
            }
        }
    }
    result
}

fn source_kept(reason: &str) -> ObjectOperationOutcome {
    ObjectOperationOutcome::SourceKept {
        reason: reason.to_owned(),
    }
}

fn validate_key(key: &str) -> Result<(), AppError> {
    if key.is_empty() {
        return Err(invalid("The object key is empty."));
    }
    check_len(key, "object key")
}

const IMAGE_EXTENSIONS: [&str; 9] = [
    "png", "jpg", "jpeg", "gif", "webp", "svg", "avif", "bmp", "ico",
];
const TEXT_EXTENSIONS: [&str; 27] = [
    "txt", "md", "markdown", "json", "jsonl", "ndjson", "csv", "tsv", "log", "xml", "yaml", "yml",
    "toml", "ini", "conf", "env", "html", "htm", "css", "js", "mjs", "ts", "sql", "sh", "py", "rs",
    "go",
];

/// Decides how an object can be previewed: by extension first, then by its
/// stored content type. `None` means no preview.
/// The type the frontend renders the preview as. It follows the preview kind,
/// not the object's stored Content-Type, which anyone with write access to the
/// bucket controls: a `.pdf` stored as `text/html` must never load as HTML.
fn preview_content_type(kind: PreviewKind, key: &str, stored: Option<&str>) -> String {
    let stored = stored
        .map(|t| {
            t.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        })
        .filter(|t| !t.is_empty());
    match kind {
        PreviewKind::Pdf => "application/pdf".to_owned(),
        PreviewKind::Image => stored
            .filter(|t| PREVIEW_IMAGE_TYPES.contains(&t.as_str()))
            .or_else(|| {
                mime_guess::from_path(key)
                    .first_raw()
                    .filter(|t| PREVIEW_IMAGE_TYPES.contains(t))
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "application/octet-stream".to_owned()),
        // Text is decoded and shown as plain text; the type is only a label.
        PreviewKind::Text => stored
            .filter(|t| t.starts_with("text/") || t.ends_with("json") || t.ends_with("xml"))
            .unwrap_or_else(|| "text/plain".to_owned()),
    }
}

fn preview_kind(key: &str, content_type: Option<&str>) -> Option<PreviewKind> {
    let name = key.rsplit('/').next().unwrap_or(key);
    let extension = name
        .rfind('.')
        .filter(|i| *i > 0)
        .map(|i| name[i + 1..].to_ascii_lowercase());
    match extension.as_deref() {
        Some(ext) if IMAGE_EXTENSIONS.contains(&ext) => return Some(PreviewKind::Image),
        Some("pdf") => return Some(PreviewKind::Pdf),
        Some(ext) if TEXT_EXTENSIONS.contains(&ext) => return Some(PreviewKind::Text),
        _ => {}
    }
    let content_type = content_type?.to_ascii_lowercase();
    if content_type.starts_with("image/") {
        Some(PreviewKind::Image)
    } else if content_type == "application/pdf" {
        Some(PreviewKind::Pdf)
    } else if content_type.starts_with("text/")
        || content_type == "application/json"
        || content_type == "application/xml"
    {
        Some(PreviewKind::Text)
    } else {
        None
    }
}

fn validate_share_duration(seconds: u32) -> Result<u32, AppError> {
    if (MIN_SHARE_SECONDS..=MAX_SHARE_SECONDS).contains(&seconds) {
        Ok(seconds)
    } else {
        Err(invalid(
            "A link must be valid for between 1 minute and 7 days.",
        ))
    }
}

fn validate_folder_name(name: &str) -> Result<&str, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(invalid("Enter a folder name."));
    }
    if name.contains('/') || name == "." || name == ".." || name.chars().any(char::is_control) {
        return Err(invalid(
            "A folder name cannot contain \"/\" or be \".\" or \"..\".",
        ));
    }
    Ok(name)
}

fn check_len(value: &str, what: &str) -> Result<(), AppError> {
    if value.len() > MAX_KEY_LEN {
        return Err(invalid(format!(
            "The {what} is longer than {MAX_KEY_LEN} bytes."
        )));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_folder_names() {
        assert_eq!(validate_folder_name("  reports ").unwrap(), "reports");
        for bad in ["", "  ", "a/b", ".", "..", "tab\there"] {
            assert!(validate_folder_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn detects_preview_kinds() {
        assert_eq!(preview_kind("a/photo.JPG", None), Some(PreviewKind::Image));
        assert_eq!(preview_kind("deck.pdf", None), Some(PreviewKind::Pdf));
        assert_eq!(
            preview_kind("data/export.csv", None),
            Some(PreviewKind::Text)
        );
        assert_eq!(
            preview_kind("noext", Some("text/plain")),
            Some(PreviewKind::Text)
        );
        assert_eq!(
            preview_kind("blob", Some("image/heic")),
            Some(PreviewKind::Image)
        );
        assert_eq!(preview_kind("archive.zip", Some("application/zip")), None);
        assert_eq!(
            preview_kind(".env", None),
            None,
            "dotfiles have no extension"
        );
    }

    #[test]
    fn validates_folder_relocation_paths() {
        assert!(validate_relocate_prefixes("photos/", "archive/photos/").is_ok());
        assert!(validate_relocate_prefixes("photos/", "pictures/").is_ok());
        assert!(validate_relocate_prefixes("photos/", "photos/").is_err());
        assert!(validate_relocate_prefixes("photos/", "photos/2026/").is_err());
        assert!(validate_relocate_prefixes("photos/", "/").is_err());
        assert!(validate_relocate_prefixes("", "a/").is_err());
        assert!(validate_relocate_prefixes("photos", "a/").is_err());
        assert!(validate_relocate_prefixes("photos/", "a").is_err());
        // "photos2/" is a sibling, not a subfolder of "photos/".
        assert!(validate_relocate_prefixes("photos/", "photos2/").is_ok());
    }

    #[test]
    fn preview_type_never_follows_a_dangerous_stored_type() {
        use PreviewKind::*;
        assert_eq!(
            preview_content_type(Pdf, "a.pdf", Some("text/html")),
            "application/pdf"
        );
        assert_eq!(
            preview_content_type(Image, "a.png", Some("text/html; charset=utf-8")),
            "image/png"
        );
        assert_eq!(
            preview_content_type(Image, "photo", Some("application/xhtml+xml")),
            "application/octet-stream"
        );
        assert_eq!(
            preview_content_type(Image, "a.jpg", Some("IMAGE/JPEG")),
            "image/jpeg"
        );
        assert_eq!(
            preview_content_type(Image, "logo.svg", None),
            "image/svg+xml"
        );
        assert_eq!(
            preview_content_type(Text, "a.txt", Some("text/html")),
            "text/html"
        );
        assert_eq!(
            preview_content_type(Text, "a.log", Some("application/x-sh")),
            "text/plain"
        );
    }

    #[test]
    fn share_duration_is_bounded_by_sigv4_limit() {
        assert!(validate_share_duration(59).is_err());
        assert!(validate_share_duration(60).is_ok());
        assert!(validate_share_duration(7 * 24 * 3600).is_ok());
        assert!(validate_share_duration(7 * 24 * 3600 + 1).is_err());
    }

    #[test]
    fn validates_key_length() {
        assert!(validate_key("").is_err());
        assert!(validate_key(&"k".repeat(1024)).is_ok());
        assert!(validate_key(&"k".repeat(1025)).is_err());
    }
}

/// Data-safety checks against a real S3-compatible server when
/// `BUCKLANE_S3_TEST_ENDPOINT` is set; skipped otherwise.
#[cfg(test)]
mod s3_integration_tests {
    use aws_sdk_s3::primitives::ByteStream;
    use sqlx::SqlitePool;

    use super::*;
    use crate::database::test_pool;
    use crate::domain::connection::{ConnectionConfigInput, ProviderType};
    use crate::providers::s3_client_factory::ClientCache;
    use crate::security::SecretString;
    use crate::security::credential_store::memory::MemoryCredentialStore;

    struct Fixture {
        pool: SqlitePool,
        store: MemoryCredentialStore,
        clients: ClientCache,
        plans: DeletePlanRegistry,
        relocations: RelocatePlanRegistry,
        connection_id: String,
        bucket: String,
    }

    impl Fixture {
        async fn new() -> Option<Self> {
            let Ok(endpoint) = std::env::var("BUCKLANE_S3_TEST_ENDPOINT") else {
                eprintln!("skipped: BUCKLANE_S3_TEST_ENDPOINT not set");
                return None;
            };
            let pool = test_pool().await;
            let store = MemoryCredentialStore::default();
            let clients = ClientCache::default();
            let connection_id = ConnectionService::new(&pool, &store, &clients)
                .create(ConnectionConfigInput {
                    name: "MinIO".into(),
                    provider: ProviderType::Minio,
                    endpoint: Some(endpoint),
                    region: "us-east-1".into(),
                    access_key_id: "bucklane-test".into(),
                    secret_access_key: Some(SecretString::new("bucklane-test-secret")),
                    force_path_style: true,
                    allow_insecure_http: true,
                })
                .await
                .unwrap()
                .id;
            let fixture = Self {
                pool,
                store,
                clients,
                plans: DeletePlanRegistry::default(),
                relocations: RelocatePlanRegistry::default(),
                connection_id,
                bucket: format!("bucklane-ops-{}", uuid::Uuid::now_v7().simple()),
            };
            fixture
                .raw()
                .await
                .create_bucket()
                .bucket(&fixture.bucket)
                .send()
                .await
                .unwrap();
            Some(fixture)
        }

        fn connections(&self) -> ConnectionService<'_> {
            ConnectionService::new(&self.pool, &self.store, &self.clients)
        }

        fn service(&self) -> ObjectService<'_> {
            ObjectService::new(self.connections(), &self.plans, &self.relocations)
        }

        async fn raw(&self) -> aws_sdk_s3::Client {
            self.connections()
                .client(&self.connection_id)
                .await
                .unwrap()
        }

        async fn put(&self, key: &str, body: &'static [u8]) {
            self.raw()
                .await
                .put_object()
                .bucket(&self.bucket)
                .key(key)
                .body(ByteStream::from_static(body))
                .send()
                .await
                .unwrap();
        }

        async fn exists(&self, key: &str) -> bool {
            self.service()
                .head_object(&self.connection_id, &self.bucket, key)
                .await
                .is_ok()
        }

        fn copy_input(&self, from: &str, to: &str, policy: ConflictPolicy) -> CopyObjectInput {
            CopyObjectInput {
                connection_id: self.connection_id.clone(),
                bucket: self.bucket.clone(),
                source_key: from.into(),
                destination_key: to.into(),
                conflict_policy: policy,
            }
        }
    }

    #[tokio::test]
    async fn create_folder_rejects_existing_folder() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        let input = CreateFolderInput {
            connection_id: f.connection_id.clone(),
            bucket: f.bucket.clone(),
            prefix: String::new(),
            name: "reports".into(),
        };

        assert_eq!(
            f.service().create_folder(input.clone()).await.unwrap(),
            "reports/"
        );
        assert!(f.exists("reports/").await);
        assert!(f.service().create_folder(input).await.is_err());
    }

    #[tokio::test]
    async fn copy_asks_before_overwriting() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        f.put("a.txt", b"original").await;
        f.put("b.txt", b"existing").await;

        let outcome = f
            .service()
            .copy_object(f.copy_input("a.txt", "b.txt", ConflictPolicy::Ask))
            .await
            .unwrap();
        assert!(matches!(
            outcome,
            ObjectOperationOutcome::Conflict {
                existing_size: Some(8),
                ..
            }
        ));

        let outcome = f
            .service()
            .copy_object(f.copy_input("a.txt", "b.txt", ConflictPolicy::Overwrite))
            .await
            .unwrap();
        assert!(matches!(outcome, ObjectOperationOutcome::Completed));
        assert!(f.exists("a.txt").await, "copy keeps the source");
    }

    #[tokio::test]
    async fn move_deletes_source_only_after_copy() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        f.put("docs/report draft.txt", b"content").await;

        let outcome = f
            .service()
            .move_object(f.copy_input(
                "docs/report draft.txt",
                "archive/report.txt",
                ConflictPolicy::Ask,
            ))
            .await
            .unwrap();

        assert!(matches!(outcome, ObjectOperationOutcome::Completed));
        assert!(f.exists("archive/report.txt").await);
        assert!(!f.exists("docs/report draft.txt").await);
    }

    #[tokio::test]
    async fn move_of_missing_source_changes_nothing() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        let err = f
            .service()
            .move_object(f.copy_input("missing.txt", "elsewhere.txt", ConflictPolicy::Ask))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::ObjectNotFound);
        assert!(!f.exists("elsewhere.txt").await);
    }

    #[tokio::test]
    async fn preview_loads_only_the_start_of_large_text_files() {
        use base64::Engine;

        let Some(f) = Fixture::new().await else {
            return;
        };
        let big = "0123456789abcdef".repeat(40 * 1024); // 640 KiB
        f.raw()
            .await
            .put_object()
            .bucket(&f.bucket)
            .key("logs/app.log")
            .body(ByteStream::from(big.clone().into_bytes()))
            .send()
            .await
            .unwrap();
        f.put("img/pixel.png", b"\x89PNG fake").await;
        f.put("bundle.zip", b"PK").await;

        let text = f
            .service()
            .preview_object(&f.connection_id, &f.bucket, "logs/app.log")
            .await
            .unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&text.data_base64)
            .unwrap();
        assert_eq!(text.kind, PreviewKind::Text);
        assert!(text.truncated);
        assert_eq!(decoded.len(), TEXT_PREVIEW_BYTES as usize);
        assert_eq!(decoded, &big.as_bytes()[..TEXT_PREVIEW_BYTES as usize]);

        let image = f
            .service()
            .preview_object(&f.connection_id, &f.bucket, "img/pixel.png")
            .await
            .unwrap();
        assert_eq!(image.kind, PreviewKind::Image);
        assert!(!image.truncated);

        let err = f
            .service()
            .preview_object(&f.connection_id, &f.bucket, "bundle.zip")
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::OperationUnsupported);
    }

    /// The link must download the object without any credentials.
    #[tokio::test]
    async fn share_link_downloads_without_credentials() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        f.put("reports/q3.txt", b"quarterly numbers").await;

        let link = f
            .service()
            .create_share_link(CreateShareLinkInput {
                connection_id: f.connection_id.clone(),
                bucket: f.bucket.clone(),
                key: "reports/q3.txt".into(),
                expires_in_seconds: 300,
            })
            .await
            .unwrap();

        let output = std::process::Command::new("curl")
            .args(["-sf", &link.url])
            .output()
            .expect("curl is available");
        assert!(output.status.success(), "link request failed");
        assert_eq!(output.stdout, b"quarterly numbers");

        let missing = f
            .service()
            .create_share_link(CreateShareLinkInput {
                connection_id: f.connection_id.clone(),
                bucket: f.bucket.clone(),
                key: "reports/missing.txt".into(),
                expires_in_seconds: 300,
            })
            .await
            .unwrap_err();
        assert_eq!(missing.code, ErrorCode::ObjectNotFound);
    }

    #[tokio::test]
    async fn prefix_delete_removes_only_snapshotted_keys() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        for key in ["tmp/", "tmp/a.txt", "tmp/nested/b.txt", "keep.txt"] {
            f.put(key, b"x").await;
        }

        let plan = f
            .service()
            .prepare_prefix_delete(&f.connection_id, &f.bucket, "tmp/")
            .await
            .unwrap();
        assert_eq!(plan.object_count, 3);
        assert_eq!(plan.total_bytes, 3);

        // Added after confirmation: must survive.
        f.put("tmp/late.txt", b"x").await;

        let result = f
            .service()
            .execute_prefix_delete(&plan.plan_id)
            .await
            .unwrap();
        assert_eq!(result.deleted.len(), 3);
        assert!(result.failed.is_empty());
        assert!(f.exists("tmp/late.txt").await);
        assert!(f.exists("keep.txt").await);
        assert!(
            f.service()
                .execute_prefix_delete(&plan.plan_id)
                .await
                .is_err(),
            "single use"
        );
    }

    fn relocate_input(
        f: &Fixture,
        from: &str,
        to: &str,
        mode: RelocateMode,
    ) -> PrefixRelocateInput {
        PrefixRelocateInput {
            connection_id: f.connection_id.clone(),
            bucket: f.bucket.clone(),
            source_prefix: from.into(),
            destination_prefix: to.into(),
            mode,
        }
    }

    #[tokio::test]
    async fn folder_move_relocates_everything_and_removes_originals() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        for key in [
            "photos/",
            "photos/a.jpg",
            "photos/2026/b.jpg",
            "photos2/keep.jpg",
        ] {
            f.put(key, b"img").await;
        }

        let plan = f
            .service()
            .prepare_prefix_relocate(relocate_input(
                &f,
                "photos/",
                "archive/photos/",
                RelocateMode::Move,
            ))
            .await
            .unwrap();
        assert_eq!(plan.object_count, 3);
        assert_eq!(plan.conflict_count, 0);

        let progress = std::sync::Mutex::new(Vec::new());
        let result = f
            .service()
            .execute_prefix_relocate(&plan.plan_id, ExistingObjectPolicy::Skip, |done, total| {
                progress.lock().unwrap().push((done, total));
            })
            .await
            .unwrap();
        assert_eq!(result.completed, 3);
        assert!(result.failed.is_empty() && result.sources_kept.is_empty());
        assert_eq!(progress.lock().unwrap().last(), Some(&(3, 3)));

        for key in [
            "archive/photos/",
            "archive/photos/a.jpg",
            "archive/photos/2026/b.jpg",
        ] {
            assert!(f.exists(key).await, "{key} should exist");
        }
        for key in ["photos/a.jpg", "photos/2026/b.jpg"] {
            assert!(!f.exists(key).await, "{key} should be gone");
        }
        assert!(
            f.exists("photos2/keep.jpg").await,
            "sibling folder untouched"
        );
    }

    #[tokio::test]
    async fn folder_copy_reports_and_skips_existing_objects() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        for key in ["src/a.txt", "src/b.txt", "dst/a.txt"] {
            f.put(key, b"x").await;
        }

        let plan = f
            .service()
            .prepare_prefix_relocate(relocate_input(&f, "src/", "dst/", RelocateMode::Copy))
            .await
            .unwrap();
        assert_eq!(plan.conflict_count, 1);
        assert_eq!(plan.conflict_samples, ["dst/a.txt"]);

        let result = f
            .service()
            .execute_prefix_relocate(&plan.plan_id, ExistingObjectPolicy::Skip, |_, _| {})
            .await
            .unwrap();
        assert_eq!((result.completed, result.skipped), (1, 1));
        assert!(f.exists("dst/b.txt").await);
        assert!(
            f.exists("src/a.txt").await && f.exists("src/b.txt").await,
            "copy keeps sources"
        );
    }

    #[tokio::test]
    async fn folder_relocate_rejects_moving_into_itself() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        f.put("docs/a.txt", b"x").await;
        let err = f
            .service()
            .prepare_prefix_relocate(relocate_input(&f, "docs/", "docs/old/", RelocateMode::Move))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
    }
}
