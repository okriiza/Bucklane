use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use aws_sdk_s3::Client;
use aws_sdk_s3::primitives::{ByteStream, SdkBody};
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use futures_util::{StreamExt, TryStreamExt, stream};

use crate::domain::object::ObjectHeaders;
use crate::domain::transfer::StoredTransfer;
use crate::errors::{AppError, ErrorCode};
use crate::providers::s3_provider::{S3StorageProvider, map_sdk_error};
use crate::providers::storage_provider::StorageProvider;
use crate::repositories::transfer_repository::TransferRepository;

use super::Progress;
use super::body::counting_file_body;

const MIB: u64 = 1024 * 1024;
/// S3 limits: parts ≥ 5 MiB (except the last) and at most 10,000 parts.
const MIN_PART_SIZE: u64 = 5 * MIB;
const MAX_PARTS: u64 = 10_000;
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
    let size = tokio::fs::metadata(&path)
        .await
        .ok()
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::LocalFileError,
                "The file no longer exists or cannot be read.",
            )
        })?;
    t.bytes_total = i64::try_from(size).ok();
    repo.update(t).await?;

    let provider = S3StorageProvider::new(client.clone());
    ensure_destination_free(&provider, t).await?;

    let headers = ObjectHeaders {
        content_type: mime_guess::from_path(&path).first_raw().map(str::to_owned),
        ..ObjectHeaders::default()
    };
    let body = |offset, len, counter| counting_file_body(path.clone(), offset, len, counter);
    write_object(client, repo, t, size, &headers, progress, tuning, &body).await?;
    verify_size(
        &provider,
        t,
        size,
        "The uploaded object's size does not match the local file.",
    )
    .await
}

/// Builds the body for `len` bytes starting at `offset`, counting into the
/// given counter.
pub(super) type BodyFactory<'a> = dyn Fn(u64, u64, Arc<AtomicU64>) -> SdkBody + Sync + 'a;

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
/// a multipart upload at or above the threshold.
#[allow(clippy::too_many_arguments)]
pub(super) async fn write_object(
    client: &Client,
    repo: &TransferRepository,
    t: &mut StoredTransfer,
    size: u64,
    headers: &ObjectHeaders,
    progress: &Progress,
    tuning: UploadTuning,
    body: &BodyFactory<'_>,
) -> Result<(), AppError> {
    if size < tuning.threshold {
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
        multipart(client, repo, t, size, part_size, headers, progress, body).await
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

/// Part size ≥ the preferred size, large enough to stay within 10,000
/// parts, and rounded up to a whole MiB.
pub(super) fn part_size_for(size: u64, preferred: u64) -> u64 {
    let needed = size.div_ceil(MAX_PARTS);
    needed.max(preferred).max(MIN_PART_SIZE).div_ceil(MIB) * MIB
}

#[allow(clippy::too_many_arguments)]
async fn multipart(
    client: &Client,
    repo: &TransferRepository,
    t: &mut StoredTransfer,
    size: u64,
    part_size: u64,
    headers: &ObjectHeaders,
    progress: &Progress,
    body: &BodyFactory<'_>,
) -> Result<(), AppError> {
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
    // Persist immediately so a crash or failure can still abort it.
    t.multipart_upload_id = Some(upload_id.clone());
    repo.update(t).await?;

    let part_count = size.div_ceil(part_size);
    let transfer_id = t.id.clone();
    let (bucket, key) = (t.bucket.clone(), t.object_key.clone());

    let mut parts: Vec<CompletedPart> = stream::iter(1..=part_count)
        .map(|n| {
            let offset = (n - 1) * part_size;
            let len = part_size.min(size - offset);
            let body = body(offset, len, progress.counter());
            let (bucket, key, upload_id, transfer_id) = (
                bucket.clone(),
                key.clone(),
                upload_id.clone(),
                transfer_id.clone(),
            );
            async move {
                let part_number = i32::try_from(n).expect("at most 10,000 parts");
                let output = client
                    .upload_part()
                    .bucket(&bucket)
                    .key(&key)
                    .upload_id(&upload_id)
                    .part_number(part_number)
                    .content_length(i64::try_from(len).unwrap_or(i64::MAX))
                    .body(ByteStream::new(body))
                    .send()
                    .await
                    .map_err(|e| map_sdk_error(e, ErrorCode::BucketNotFound))?;
                let etag = output
                    .e_tag()
                    .ok_or_else(|| AppError::internal("The provider did not return a part ETag."))?
                    .to_owned();
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
    repo.delete_parts(&t.id).await?;
    repo.update(t).await?;
    Ok(())
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

    #[test]
    fn part_size_respects_s3_limits() {
        let preferred = 16 * MIB;
        assert_eq!(part_size_for(100 * MIB, preferred), preferred);
        // Never below the S3 minimum, whatever the preference.
        assert_eq!(part_size_for(100 * MIB, MIB), MIN_PART_SIZE);
        // 1 TiB / 10,000 parts ≈ 105 MiB → rounded up to whole MiB.
        let tib = 1024 * 1024 * MIB;
        let size = part_size_for(tib, preferred);
        assert!(tib.div_ceil(size) <= MAX_PARTS);
        assert_eq!(size % MIB, 0);
        // 5 TiB (S3 maximum object size) still fits.
        assert!((5 * tib).div_ceil(part_size_for(5 * tib, preferred)) <= MAX_PARTS);
    }
}
