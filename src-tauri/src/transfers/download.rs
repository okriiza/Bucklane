use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use aws_sdk_s3::Client;
use tokio::io::AsyncWriteExt;

use crate::domain::transfer::StoredTransfer;
use crate::errors::{AppError, ErrorCode};
use crate::providers::s3_provider::{S3StorageProvider, map_sdk_error};
use crate::providers::storage_provider::StorageProvider;
use crate::repositories::transfer_repository::TransferRepository;

use super::Progress;

/// Safe download (TSD §9.4): stream into a temporary file next to the
/// destination, verify the size, then finalize. A partial file is never
/// presented as the downloaded file, and an existing file is only replaced
/// when the user chose to overwrite it.
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
    repo.update(t).await?;

    // Folder downloads recreate subfolders; `join_relative` already confined
    // the path to the user's chosen destination.
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(map_io_error)?;
    }

    // A leftover from an earlier attempt is ours to replace.
    let _ = tokio::fs::remove_file(&temp).await;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .await
        .map_err(map_io_error)?;

    // If-Match pins the object version we measured, so a concurrent change
    // fails the download instead of mixing contents.
    let mut body = client
        .get_object()
        .bucket(&t.bucket)
        .key(&t.object_key)
        .set_if_match(metadata.etag.clone())
        .send()
        .await
        .map_err(|e| map_sdk_error(e, ErrorCode::ObjectNotFound))?
        .body;

    let counter = progress.counter();
    while let Some(chunk) = body.try_next().await.map_err(|err| {
        AppError::new(
            ErrorCode::TransferFailed,
            "The connection was interrupted while downloading.",
        )
        .with_details(err.to_string())
    })? {
        file.write_all(&chunk).await.map_err(map_io_error)?;
        counter.fetch_add(chunk.len() as u64, Ordering::Relaxed);
    }
    file.flush().await.map_err(map_io_error)?;
    file.sync_all().await.map_err(map_io_error)?;
    drop(file);

    let written = i64::try_from(counter.load(Ordering::Relaxed)).unwrap_or(i64::MAX);
    if written != expected {
        return Err(AppError::new(
            ErrorCode::TransferFailed,
            "The download was incomplete.",
        ));
    }

    finalize(&temp, &destination, t.overwrite).await
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
