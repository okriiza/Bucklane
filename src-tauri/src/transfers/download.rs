use std::io::{ErrorKind, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use aws_sdk_s3::Client;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

use crate::domain::transfer::StoredTransfer;
use crate::errors::{AppError, ErrorCode};
use crate::providers::s3_provider::{S3StorageProvider, map_sdk_error};
use crate::providers::storage_provider::StorageProvider;
use crate::repositories::transfer_repository::TransferRepository;

use super::Progress;

/// How often a running download makes its progress durable (fsync) and
/// records it as the point a later attempt can resume from.
const CHECKPOINT_INTERVAL: Duration = Duration::from_secs(2);

/// Safe, resumable download (TSD §9.4): stream into a temporary file next to
/// the destination, verify the size, then finalize. A partial file is never
/// presented as the downloaded file, and an existing file is only replaced
/// when the user chose to overwrite it.
///
/// The temporary file survives failures, app restarts, and power loss. A
/// retry continues from the last checkpoint if the object is unchanged (same
/// ETag); otherwise it starts over. Bytes after the checkpoint may not have
/// reached the disk, so they are cut off and downloaded again.
pub(super) async fn run(
    client: &Client,
    repo: &TransferRepository,
    t: &mut StoredTransfer,
    progress: &Progress,
) -> Result<(), AppError> {
    let destination = PathBuf::from(&t.local_path);
    let temp = temp_path(t).ok_or_else(|| local_error("The destination folder is not valid."))?;

    let metadata = S3StorageProvider::new(client.clone())
        .head_object(&t.bucket, &t.object_key)
        .await?;
    let expected = metadata.size.unwrap_or(0);
    t.bytes_total = Some(expected);

    // Folder downloads recreate subfolders; `join_relative` already confined
    // the path to the user's chosen destination.
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(map_io_error)?;
    }

    let temp_len = tokio::fs::metadata(&temp).await.ok().map(|m| m.len());
    let mut offset = resume_point(
        t.resume_etag.as_deref(),
        t.resume_offset,
        metadata.etag.as_deref(),
        temp_len,
        expected,
    );
    let mut file = if offset > 0 {
        tracing::info!(transfer_id = %t.id, offset, "resuming download");
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .open(&temp)
            .await
            .map_err(map_io_error)?;
        file.set_len(offset).await.map_err(map_io_error)?;
        file.seek(SeekFrom::Start(offset))
            .await
            .map_err(map_io_error)?;
        file
    } else {
        // A leftover from an earlier attempt is ours to replace.
        let _ = tokio::fs::remove_file(&temp).await;
        tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .await
            .map_err(map_io_error)?
    };
    t.resume_etag = metadata.etag.clone();
    t.resume_offset = offset as i64;
    repo.update(t).await?;

    let counter = progress.counter();
    counter.store(offset, Ordering::Relaxed);

    if offset < expected as u64 || expected == 0 {
        // If-Match pins the object version we measured, so a concurrent
        // change fails the download instead of mixing contents.
        let output = client
            .get_object()
            .bucket(&t.bucket)
            .key(&t.object_key)
            .set_if_match(metadata.etag.clone())
            .set_range((offset > 0).then(|| format!("bytes={offset}-")))
            .send()
            .await
            .map_err(|e| map_sdk_error(e, ErrorCode::ObjectNotFound))?;

        // A server that ignores Range sends the whole object: start over.
        if offset > 0 && output.content_range().is_none() {
            tracing::info!(transfer_id = %t.id, "server ignored the range; restarting download");
            offset = 0;
            file.set_len(0).await.map_err(map_io_error)?;
            file.seek(SeekFrom::Start(0)).await.map_err(map_io_error)?;
            counter.store(0, Ordering::Relaxed);
            t.resume_offset = 0;
            repo.update_resume_offset(&t.id, 0).await?;
        }

        let mut body = output.body;
        let mut written = offset;
        let mut last_checkpoint = Instant::now();
        while let Some(chunk) = body.try_next().await.map_err(|err| {
            AppError::new(
                ErrorCode::TransferFailed,
                "The connection was interrupted while downloading. Retry to resume.",
            )
            .with_details(err.to_string())
        })? {
            file.write_all(&chunk).await.map_err(map_io_error)?;
            written += chunk.len() as u64;
            counter.store(written, Ordering::Relaxed);
            if last_checkpoint.elapsed() >= CHECKPOINT_INTERVAL {
                file.flush().await.map_err(map_io_error)?;
                file.sync_data().await.map_err(map_io_error)?;
                t.resume_offset = written as i64;
                repo.update_resume_offset(&t.id, t.resume_offset).await?;
                last_checkpoint = Instant::now();
            }
        }
    }
    file.flush().await.map_err(map_io_error)?;
    file.sync_all().await.map_err(map_io_error)?;
    drop(file);

    let written = i64::try_from(counter.load(Ordering::Relaxed)).unwrap_or(i64::MAX);
    t.resume_offset = written;
    if written != expected {
        return Err(AppError::new(
            ErrorCode::TransferFailed,
            "The download was incomplete. Retry to resume.",
        ));
    }

    finalize(&temp, &destination, t.overwrite).await?;
    t.resume_etag = None;
    t.resume_offset = 0;
    Ok(())
}

/// Where a retry can continue: the recorded checkpoint, if the temporary file
/// still holds it and the object has not changed since. 0 means start over.
fn resume_point(
    stored_etag: Option<&str>,
    stored_offset: i64,
    current_etag: Option<&str>,
    temp_len: Option<u64>,
    expected: i64,
) -> u64 {
    let same_object = matches!((stored_etag, current_etag), (Some(a), Some(b)) if a == b);
    let (Ok(offset), Some(len), Ok(expected)) = (
        u64::try_from(stored_offset),
        temp_len,
        u64::try_from(expected),
    ) else {
        return 0;
    };
    if same_object && offset <= len && offset <= expected {
        offset
    } else {
        0
    }
}

async fn finalize(temp: &Path, destination: &Path, overwrite: bool) -> Result<(), AppError> {
    if overwrite {
        // rename() atomically replaces the destination on the same volume.
        return tokio::fs::rename(temp, destination)
            .await
            .map_err(map_io_error);
    }
    // A hard link fails if the destination appeared in the meantime, closing
    // the check-then-rename race on filesystems that support links.
    match tokio::fs::hard_link(temp, destination).await {
        Ok(()) => {
            let _ = tokio::fs::remove_file(temp).await;
            Ok(())
        }
        Err(err) if err.kind() == ErrorKind::AlreadyExists => Err(exists_error()),
        Err(_) => {
            if tokio::fs::try_exists(destination).await.unwrap_or(true) {
                return Err(exists_error());
            }
            tokio::fs::rename(temp, destination)
                .await
                .map_err(map_io_error)
        }
    }
}

/// Hidden temporary file in the destination folder, unique per transfer.
pub(super) fn temp_path(t: &StoredTransfer) -> Option<PathBuf> {
    let destination = Path::new(&t.local_path);
    let dir = destination.parent()?;
    let name = destination.file_name()?.to_string_lossy();
    Some(dir.join(format!(".{name}.{}.bucklane-part", t.id)))
}

pub(super) async fn remove_temp_file(t: &StoredTransfer) {
    if let Some(temp) = temp_path(t) {
        let _ = tokio::fs::remove_file(temp).await;
    }
}

fn exists_error() -> AppError {
    local_error("A file with this name now exists in the destination folder. It was not replaced.")
}

fn local_error(message: &str) -> AppError {
    AppError::new(ErrorCode::LocalFileError, message)
}

fn map_io_error(err: std::io::Error) -> AppError {
    let message = match err.kind() {
        ErrorKind::StorageFull => "There is not enough free disk space.",
        ErrorKind::PermissionDenied => "Bucklane does not have permission to write to this folder.",
        ErrorKind::NotFound => "The destination folder no longer exists.",
        _ => "The file could not be written.",
    };
    AppError::new(ErrorCode::LocalFileError, message).with_details(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bucklane-dl-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resumes_only_an_unchanged_object_from_a_valid_checkpoint() {
        let etag = Some("\"abc\"");
        // Same object, checkpoint inside the temp file.
        assert_eq!(resume_point(etag, 400, etag, Some(500), 1000), 400);
        // Complete but not yet finalized.
        assert_eq!(resume_point(etag, 1000, etag, Some(1000), 1000), 1000);
        // Object changed on the server.
        assert_eq!(resume_point(etag, 400, Some("\"new\""), Some(500), 1000), 0);
        // No version information on either side.
        assert_eq!(resume_point(None, 400, None, Some(500), 1000), 0);
        // Temp file missing or shorter than the checkpoint (e.g. power loss).
        assert_eq!(resume_point(etag, 400, etag, None, 1000), 0);
        assert_eq!(resume_point(etag, 400, etag, Some(300), 1000), 0);
        // Checkpoint past the object's size.
        assert_eq!(resume_point(etag, 1200, etag, Some(1200), 1000), 0);
        assert_eq!(resume_point(etag, -1, etag, Some(10), 1000), 0);
    }

    #[tokio::test]
    async fn finalize_never_replaces_without_overwrite() {
        let dir = scratch();
        let (temp, dest) = (dir.join(".tmp"), dir.join("file.txt"));
        std::fs::write(&temp, b"new").unwrap();
        std::fs::write(&dest, b"original").unwrap();

        let err = finalize(&temp, &dest, false).await.unwrap_err();

        assert_eq!(err.code, ErrorCode::LocalFileError);
        assert_eq!(std::fs::read(&dest).unwrap(), b"original");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn finalize_moves_temp_into_place() {
        let dir = scratch();
        let (temp, dest) = (dir.join(".tmp"), dir.join("file.txt"));
        std::fs::write(&temp, b"data").unwrap();

        finalize(&temp, &dest, false).await.unwrap();

        assert_eq!(std::fs::read(&dest).unwrap(), b"data");
        assert!(!temp.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn finalize_overwrites_when_chosen() {
        let dir = scratch();
        let (temp, dest) = (dir.join(".tmp"), dir.join("file.txt"));
        std::fs::write(&temp, b"new").unwrap();
        std::fs::write(&dest, b"old").unwrap();

        finalize(&temp, &dest, true).await.unwrap();

        assert_eq!(std::fs::read(&dest).unwrap(), b"new");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
