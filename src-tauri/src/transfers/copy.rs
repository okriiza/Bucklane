use std::sync::{Arc, Mutex};

use aws_sdk_s3::Client;

use crate::domain::object::ObjectHeaders;
use crate::domain::transfer::StoredTransfer;
use crate::errors::{AppError, ErrorCode};
use crate::providers::s3_provider::S3StorageProvider;
use crate::providers::storage_provider::StorageProvider;
use crate::repositories::transfer_repository::TransferRepository;

use super::Progress;
use super::body::{ObjectRange, counting_object_body};
use super::upload::{self, UploadTuning};

/// Copies one object to another bucket or connection (TSD §9.5). Bytes are
/// streamed from ranged GETs straight into the destination's PUT or
/// multipart parts; nothing is written to disk and at most one chunk per
/// part is held in memory.
pub(super) async fn run(
    source: &Client,
    destination: &Client,
    repo: &TransferRepository,
    t: &mut StoredTransfer,
    progress: &Progress,
    tuning: UploadTuning,
) -> Result<(), AppError> {
    let from = t
        .source
        .clone()
        .ok_or_else(|| AppError::internal("This copy has no source."))?;
    let metadata = S3StorageProvider::new(source.clone())
        .head_object(&from.bucket, &from.key)
        .await?;
    let size = metadata
        .size
        .and_then(|s| u64::try_from(s).ok())
        .ok_or_else(|| AppError::internal("The source did not report the object's size."))?;
    t.bytes_total = i64::try_from(size).ok();
    repo.update(t).await?;

    let provider = S3StorageProvider::new(destination.clone());
    upload::ensure_destination_free(&provider, t).await?;

    // Storage class, ACLs, and encryption settings belong to the source
    // provider and are not carried over; the HTTP headers are.
    let headers = ObjectHeaders {
        content_type: metadata.content_type,
        cache_control: metadata.cache_control,
        content_disposition: metadata.content_disposition,
        content_encoding: metadata.content_encoding,
        content_language: metadata.content_language,
        metadata: metadata.metadata,
    };
    let failure = Arc::new(Mutex::new(None));
    let body = |offset, len, counter| {
        counting_object_body(
            ObjectRange {
                client: source.clone(),
                bucket: from.bucket.clone(),
                key: from.key.clone(),
                // Every range comes from the version measured above.
                etag: metadata.etag.clone(),
                offset,
                len,
                size,
            },
            counter,
            failure.clone(),
        )
    };
    let written = upload::write_object(
        destination,
        repo,
        t,
        size,
        &headers,
        progress,
        tuning,
        &body,
    )
    .await;
    if let Err(err) = written {
        // A failing source surfaces as a generic send error; report its cause.
        let source_error = failure.lock().expect("copy failure poisoned").take();
        return Err(source_error.map_or(err, |e| {
            AppError::new(e.code, format!("Reading the source failed: {}", e.message))
        }));
    }
    upload::verify_size(
        &provider,
        t,
        size,
        "The copy's size does not match the original.",
    )
    .await
    .map_err(|err| {
        if err.code == ErrorCode::ObjectNotFound {
            AppError::new(
                ErrorCode::TransferFailed,
                "The copy could not be found after it was written.",
            )
        } else {
            err
        }
    })
}
