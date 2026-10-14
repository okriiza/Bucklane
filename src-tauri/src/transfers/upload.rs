use std::path::{Path, PathBuf};

use aws_sdk_s3::Client;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use futures_util::{StreamExt, TryStreamExt, stream};

use crate::domain::transfer::StoredTransfer;
use crate::errors::{AppError, ErrorCode};
use crate::providers::s3_provider::{S3StorageProvider, map_sdk_error};
use crate::providers::storage_provider::StorageProvider;
use crate::repositories::transfer_repository::TransferRepository;

use super::Progress;
use super::body::counting_file_body;

const MIB: u64 = 1024 * 1024;
/// Files at or above this size use multipart upload (settings default).
const MULTIPART_THRESHOLD: u64 = 64 * MIB;
const DEFAULT_PART_SIZE: u64 = 16 * MIB;
/// S3 limits: parts ≥ 5 MiB (except the last) and at most 10,000 parts.
const MIN_PART_SIZE: u64 = 5 * MIB;
const MAX_PARTS: u64 = 10_000;
/// Parts uploaded concurrently within one transfer.
const PART_CONCURRENCY: usize = 4;

pub(super) async fn run(
    client: &Client,
    repo: &TransferRepository,
    t: &mut StoredTransfer,
    progress: &Progress,
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
    // The conflict was resolved when queued; re-check so a retry or a
    // concurrent change never silently replaces an object.
    if !t.overwrite {
        match provider.head_object(&t.bucket, &t.object_key).await {
            Ok(_) => {
                return Err(AppError::new(
                    ErrorCode::TransferFailed,
                    "An object with this name now exists at the destination. It was not replaced.",
                ));
            }
            Err(err) if err.code == ErrorCode::ObjectNotFound => {}
            Err(err) => return Err(err),
        }
    }

    let content_type = mime_guess::from_path(&path).first_raw().map(str::to_owned);
    if size < MULTIPART_THRESHOLD {
        client
            .put_object()
            .bucket(&t.bucket)
            .key(&t.object_key)
            .set_content_type(content_type)
            .content_length(i64::try_from(size).unwrap_or(i64::MAX))
            .body(ByteStream::new(counting_file_body(
                path,
                0,
                size,
                progress.counter(),
            )))
            .send()
            .await
            .map_err(|e| map_sdk_error(e, ErrorCode::BucketNotFound))?;
    } else {
        multipart(client, repo, t, &path, size, content_type, progress).await?;
    }

    // Verify the stored object has the expected size.
    let stored = provider.head_object(&t.bucket, &t.object_key).await?;
    if stored.size != i64::try_from(size).ok() {
        return Err(AppError::new(
            ErrorCode::TransferFailed,
            "The uploaded object's size does not match the local file.",
        ));
    }
    Ok(())
}

/// Part size ≥ the default, large enough to stay within 10,000 parts, and
/// rounded up to a whole MiB.
pub(super) fn part_size_for(size: u64) -> u64 {
    let needed = size.div_ceil(MAX_PARTS);
    needed
        .max(DEFAULT_PART_SIZE)
        .max(MIN_PART_SIZE)
        .div_ceil(MIB)
        * MIB
}

async fn multipart(
    client: &Client,
    repo: &TransferRepository,
    t: &mut StoredTransfer,
    path: &Path,
    size: u64,
    content_type: Option<String>,
    progress: &Progress,
) -> Result<(), AppError> {
    let created = client
        .create_multipart_upload()
        .bucket(&t.bucket)
        .key(&t.object_key)
        .set_content_type(content_type)
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

    let part_size = part_size_for(size);
    let part_count = size.div_ceil(part_size);
    let transfer_id = t.id.clone();
    let (bucket, key) = (t.bucket.clone(), t.object_key.clone());

    let mut parts: Vec<CompletedPart> = stream::iter(1..=part_count)
        .map(|n| {
            let offset = (n - 1) * part_size;
            let len = part_size.min(size - offset);
            let body = counting_file_body(path.to_path_buf(), offset, len, progress.counter());
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
        assert_eq!(part_size_for(100 * MIB), DEFAULT_PART_SIZE);
        // 1 TiB / 10,000 parts ≈ 105 MiB → rounded up to whole MiB.
        let tib = 1024 * 1024 * MIB;
        let size = part_size_for(tib);
        assert!(tib.div_ceil(size) <= MAX_PARTS);
        assert_eq!(size % MIB, 0);
        // 5 TiB (S3 maximum object size) still fits.
        assert!((5 * tib).div_ceil(part_size_for(5 * tib)) <= MAX_PARTS);
    }
}
