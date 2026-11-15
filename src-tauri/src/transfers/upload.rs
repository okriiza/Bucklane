use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use aws_sdk_s3::Client;
use aws_sdk_s3::error::ProvideErrorMetadata;
use aws_sdk_s3::primitives::{ByteStream, SdkBody};
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use futures_util::{StreamExt, TryStreamExt, stream};

use crate::domain::object::ObjectHeaders;
use crate::domain::transfer::StoredTransfer;
use crate::errors::{AppError, ErrorCode};
use crate::providers::s3_provider::{S3StorageProvider, map_sdk_error, part_size_for};
use crate::providers::storage_provider::{CopySource, StorageProvider};
use crate::repositories::transfer_repository::{StoredPart, TransferRepository};

use super::Progress;
use super::body::counting_file_body;

const MIB: u64 = 1024 * 1024;
/// Parts uploaded concurrently within one transfer.
const PART_CONCURRENCY: usize = 4;

/// Multipart parameters from settings.
#[derive(Debug, Clone, Copy)]
pub(crate) struct UploadTuning {
    /// Files at or above this size use multipart upload.
    pub threshold: u64,
    /// Preferred part size; raised automatically for very large files.
    pub part_size: u64,
}

impl UploadTuning {
    pub(crate) fn from_settings(settings: &crate::domain::settings::AppSettings) -> Self {
        Self {
            threshold: u64::from(settings.multipart_threshold_mb) * MIB,
            part_size: u64::from(settings.multipart_part_size_mb) * MIB,
        }
    }
}

impl Default for UploadTuning {
    fn default() -> Self {
        Self::from_settings(&crate::domain::settings::AppSettings::default())
    }
}

pub(super) async fn run(
    client: &Client,
    repo: &TransferRepository,
    t: &mut StoredTransfer,
    progress: &Progress,
    tuning: UploadTuning,
) -> Result<(), AppError> {
    let path = PathBuf::from(&t.local_path);
    let metadata = tokio::fs::metadata(&path)
        .await
        .ok()
        .filter(std::fs::Metadata::is_file)
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::LocalFileError,
                "The file no longer exists or cannot be read.",
            )
        })?;
    let size = metadata.len();
    t.bytes_total = i64::try_from(size).ok();
    repo.update(t).await?;

    let provider = S3StorageProvider::new(client.clone());
    ensure_destination_free(&provider, t).await?;

    let headers = ObjectHeaders {
        content_type: mime_guess::from_path(&path).first_raw().map(str::to_owned),
        ..ObjectHeaders::default()
    };
    let version = file_version(&metadata);
    let body = |offset, len, counter| counting_file_body(path.clone(), offset, len, counter);
    write_object(
        client,
        repo,
        t,
        size,
        &headers,
        version.as_deref(),
        progress,
        tuning,
        PartSource::Stream(&body),
    )
    .await?;
    verify_size(
        &provider,
        t,
        size,
        "The uploaded object's size does not match the local file.",
    )
    .await
}

/// Identifies the local file's contents for resuming: an upload continues
/// only if the file still has the same size and modification time.
pub(super) fn file_version(metadata: &std::fs::Metadata) -> Option<String> {
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    Some(format!("file:{}:{}", metadata.len(), modified.as_nanos()))
}

/// Builds the body for `len` bytes starting at `offset`, counting into the
/// given counter.
pub(super) type BodyFactory<'a> = dyn Fn(u64, u64, Arc<AtomicU64>) -> SdkBody + Sync + 'a;

/// Where the bytes of an object being written come from.
#[derive(Clone, Copy)]
pub(super) enum PartSource<'a> {
    /// Sent by Bucklane.
    Stream(&'a BodyFactory<'a>),
    /// Copied by the provider from another object on the same connection;
    /// nothing passes through Bucklane.
    Server(CopySource<'a>),
}

/// The conflict was resolved when queued; re-check so a retry or a
/// concurrent change never silently replaces an object.
pub(super) async fn ensure_destination_free(
    provider: &S3StorageProvider,
    t: &StoredTransfer,
) -> Result<(), AppError> {
    if t.overwrite {
        return Ok(());
    }
    match provider.head_object(&t.bucket, &t.object_key).await {
        Ok(_) => Err(AppError::new(
            ErrorCode::TransferFailed,
            "An object with this name now exists at the destination. It was not replaced.",
        )),
        Err(err) if err.code == ErrorCode::ObjectNotFound => Ok(()),
        Err(err) => Err(err),
    }
}

/// Stores `size` bytes at the transfer's destination with one PUT, or with
/// a multipart upload at or above the threshold. `version` identifies the
/// source's contents; a multipart upload kept from an earlier attempt of
/// the same version continues where it stopped.
#[allow(clippy::too_many_arguments)]
pub(super) async fn write_object(
    client: &Client,
    repo: &TransferRepository,
    t: &mut StoredTransfer,
    size: u64,
    headers: &ObjectHeaders,
    version: Option<&str>,
    progress: &Progress,
    tuning: UploadTuning,
    source: PartSource<'_>,
) -> Result<(), AppError> {
    if size < tuning.threshold {
        // A kept multipart upload is useless for a single request.
        discard_upload(client, repo, t).await?;
        let body = match source {
            PartSource::Stream(body) => body,
            PartSource::Server(from) => {
                // A single copy carries the source's headers over.
                S3StorageProvider::new(client.clone())
                    .copy_object(from, &t.bucket, &t.object_key)
                    .await?;
                progress.counter().store(size, Ordering::Relaxed);
                return Ok(());
            }
        };
        client
            .put_object()
            .bucket(&t.bucket)
            .key(&t.object_key)
            .set_content_type(headers.content_type.clone())
            .set_cache_control(headers.cache_control.clone())
            .set_content_disposition(headers.content_disposition.clone())
            .set_content_encoding(headers.content_encoding.clone())
            .set_content_language(headers.content_language.clone())
            .set_metadata(user_metadata(headers))
            .content_length(i64::try_from(size).unwrap_or(i64::MAX))
            .body(ByteStream::new(body(0, size, progress.counter())))
            .send()
            .await
            .map_err(|e| map_sdk_error(e, ErrorCode::BucketNotFound))?;
        Ok(())
    } else {
        let part_size = part_size_for(size, tuning.part_size);
        multipart(
            client, repo, t, size, part_size, headers, version, progress, source,
        )
        .await
    }
}

/// Fails unless the stored object has the expected size.
pub(super) async fn verify_size(
    provider: &S3StorageProvider,
    t: &StoredTransfer,
    size: u64,
    mismatch: &str,
) -> Result<(), AppError> {
    let stored = provider.head_object(&t.bucket, &t.object_key).await?;
    if stored.size != i64::try_from(size).ok() {
        return Err(AppError::new(ErrorCode::TransferFailed, mismatch));
    }
    Ok(())
}

fn user_metadata(headers: &ObjectHeaders) -> Option<std::collections::HashMap<String, String>> {
    (!headers.metadata.is_empty()).then(|| headers.metadata.clone().into_iter().collect())
}

#[allow(clippy::too_many_arguments)]
async fn multipart(
    client: &Client,
    repo: &TransferRepository,
    t: &mut StoredTransfer,
    size: u64,
    part_size: u64,
    headers: &ObjectHeaders,
    version: Option<&str>,
    progress: &Progress,
    source: PartSource<'_>,
) -> Result<(), AppError> {
    let part_count = size.div_ceil(part_size);
    let kept = kept_parts(client, repo, t, size, part_size, version).await?;
    let upload_id = match (&t.multipart_upload_id, kept.is_some()) {
        (Some(id), true) => id.clone(),
        _ => {
            discard_upload(client, repo, t).await?;
            let created = client
                .create_multipart_upload()
                .bucket(&t.bucket)
                .key(&t.object_key)
                .set_content_type(headers.content_type.clone())
                .set_cache_control(headers.cache_control.clone())
                .set_content_disposition(headers.content_disposition.clone())
                .set_content_encoding(headers.content_encoding.clone())
                .set_content_language(headers.content_language.clone())
                .set_metadata(user_metadata(headers))
                .send()
                .await
                .map_err(|e| map_sdk_error(e, ErrorCode::BucketNotFound))?;
            let upload_id = created
                .upload_id()
                .ok_or_else(|| AppError::internal("The provider did not return an upload ID."))?
                .to_owned();
            // Persist immediately so a crash or failure can still abort or
            // resume it.
            t.multipart_upload_id = Some(upload_id.clone());
            t.resume_etag = version.map(str::to_owned);
            t.resume_offset = 0;
            repo.update(t).await?;
            upload_id
        }
    };

    let kept = kept.unwrap_or_default();
    if !kept.is_empty() {
        tracing::info!(transfer_id = %t.id, parts = kept.len(), "resuming multipart upload");
    }
    let kept_bytes: u64 = kept.iter().map(|p| p.size.unsigned_abs()).sum();
    progress.counter().store(kept_bytes, Ordering::Relaxed);
    let done: HashSet<i32> = kept.iter().map(|p| p.number).collect();

    let transfer_id = t.id.clone();
    let (bucket, key) = (t.bucket.clone(), t.object_key.clone());
    let missing = (1..=part_count)
        .filter(|n| !done.contains(&i32::try_from(*n).expect("at most 10,000 parts")));

    let mut parts: Vec<CompletedPart> = stream::iter(missing)
        .map(|n| {
            let offset = (n - 1) * part_size;
            let len = part_size.min(size - offset);
            let counter = progress.counter();
            let (bucket, key, upload_id, transfer_id) = (
                bucket.clone(),
                key.clone(),
                upload_id.clone(),
                transfer_id.clone(),
            );
            async move {
                let part_number = i32::try_from(n).expect("at most 10,000 parts");
                let etag = match source {
                    PartSource::Stream(body) => client
                        .upload_part()
                        .bucket(&bucket)
                        .key(&key)
                        .upload_id(&upload_id)
                        .part_number(part_number)
                        .content_length(i64::try_from(len).unwrap_or(i64::MAX))
                        .body(ByteStream::new(body(offset, len, counter)))
                        .send()
                        .await
                        .map_err(|e| map_sdk_error(e, ErrorCode::BucketNotFound))?
                        .e_tag()
                        .map(str::to_owned),
                    PartSource::Server(from) => {
                        let part = S3StorageProvider::new(client.clone())
                            .upload_part_copy(
                                from,
                                &bucket,
                                &key,
                                &upload_id,
                                part_number,
                                offset,
                                len,
                            )
                            .await?;
                        // Copied in one step; count it when it's done.
                        counter.store(len, Ordering::Relaxed);
                        part.e_tag().map(str::to_owned)
                    }
                }
                .ok_or_else(|| AppError::internal("The provider did not return a part ETag."))?;
                repo.add_part(
                    &transfer_id,
                    part_number,
                    &etag,
                    i64::try_from(len).unwrap_or(0),
                )
                .await?;
                Ok::<_, AppError>(
                    CompletedPart::builder()
                        .part_number(part_number)
                        .e_tag(etag)
                        .build(),
                )
            }
        })
        .buffer_unordered(PART_CONCURRENCY)
        .try_collect()
        .await?;
    parts.extend(kept.into_iter().map(|p| {
        CompletedPart::builder()
            .part_number(p.number)
            .e_tag(p.etag)
            .build()
    }));

    // Completion requires parts in ascending order.
    parts.sort_by_key(|p| p.part_number());
    client
        .complete_multipart_upload()
        .bucket(&t.bucket)
        .key(&t.object_key)
        .upload_id(&upload_id)
        .multipart_upload(
            CompletedMultipartUpload::builder()
                .set_parts(Some(parts))
                .build(),
        )
        .send()
        .await
        .map_err(|e| map_sdk_error(e, ErrorCode::BucketNotFound))?;

    t.multipart_upload_id = None;
    t.resume_etag = None;
    repo.delete_parts(&t.id).await?;
    t.resume_offset = 0;
    repo.update(t).await?;
    Ok(())
}

/// The parts an earlier attempt stored, if its multipart upload can be
/// continued: same source version, same part layout, and the provider still
/// has the upload. `None` means start over.
async fn kept_parts(
    client: &Client,
    repo: &TransferRepository,
    t: &StoredTransfer,
    size: u64,
    part_size: u64,
    version: Option<&str>,
) -> Result<Option<Vec<StoredPart>>, AppError> {
    let Some(upload_id) = &t.multipart_upload_id else {
        return Ok(None);
    };
    if version.is_none() || t.resume_etag.as_deref() != version {
        return Ok(None);
    }
    let parts = repo.parts(&t.id).await?;
    if !parts_fit(&parts, size, part_size) {
        return Ok(None);
    }
    // An upload aborted elsewhere or expired by a lifecycle rule is gone;
    // so is anything the provider can't confirm.
    match client
        .list_parts()
        .bucket(&t.bucket)
        .key(&t.object_key)
        .upload_id(upload_id)
        .max_parts(1)
        .send()
        .await
    {
        Ok(_) => Ok(Some(parts)),
        Err(err) => {
            tracing::info!(
                transfer_id = %t.id,
                code = err.code().unwrap_or("unknown"),
                "kept multipart upload is unavailable; starting over"
            );
            Ok(None)
        }
    }
}

/// Every stored part has the size the current layout gives it, so a part
/// size changed in Settings since then starts the upload over.
fn parts_fit(parts: &[StoredPart], size: u64, part_size: u64) -> bool {
    let count = size.div_ceil(part_size);
    parts.iter().all(|p| {
        let Ok(n) = u64::try_from(p.number) else {
            return false;
        };
        if n == 0 || n > count {
            return false;
        }
        let offset = (n - 1) * part_size;
        u64::try_from(p.size).ok() == Some(part_size.min(size - offset))
    })
}

/// Aborts a multipart upload kept from an earlier attempt, if any.
async fn discard_upload(
    client: &Client,
    repo: &TransferRepository,
    t: &mut StoredTransfer,
) -> Result<(), AppError> {
    let Some(upload_id) = t.multipart_upload_id.clone() else {
        return Ok(());
    };
    if let Err(err) = abort(client, &t.bucket, &t.object_key, &upload_id).await {
        // Starting over matters more; the old upload just costs storage
        // until the provider expires it.
        tracing::warn!(transfer_id = %t.id, error = %err, "could not abort the earlier multipart upload");
    }
    t.multipart_upload_id = None;
    t.resume_etag = None;
    repo.delete_parts(&t.id).await?;
    t.resume_offset = 0;
    repo.update(t).await
}

pub(super) async fn abort(
    client: &Client,
    bucket: &str,
    key: &str,
    upload_id: &str,
) -> Result<(), AppError> {
    match client
        .abort_multipart_upload()
        .bucket(bucket)
        .key(key)
        .upload_id(upload_id)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(err) => {
            let err = map_sdk_error(err, ErrorCode::ObjectNotFound);
            // Already gone (completed or aborted elsewhere) counts as success.
            if err.code == ErrorCode::ObjectNotFound {
                Ok(())
            } else {
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(number: i32, size: u64) -> StoredPart {
        StoredPart {
            number,
            etag: format!("\"{number}\""),
            size: i64::try_from(size).unwrap(),
        }
    }

    #[test]
    fn kept_parts_must_match_the_current_layout() {
        let size = 12 * MIB;
        let parts = [part(1, 5 * MIB), part(3, 2 * MIB)];
        assert!(parts_fit(&parts, size, 5 * MIB));
        assert!(parts_fit(&[], size, 5 * MIB));
        // Part size changed in Settings since.
        assert!(!parts_fit(&parts, size, 6 * MIB));
        // Beyond the last part, or a short middle part.
        assert!(!parts_fit(&[part(4, MIB)], size, 5 * MIB));
        assert!(!parts_fit(&[part(2, MIB)], size, 5 * MIB));
        assert!(!parts_fit(&[part(0, 5 * MIB)], size, 5 * MIB));
    }
}
