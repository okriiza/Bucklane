use serde::{Deserialize, Serialize};
use specta::Type;

use crate::errors::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferDirection {
    Upload,
    Download,
}

impl TransferDirection {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Upload => "UPLOAD",
            Self::Download => "DOWNLOAD",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "UPLOAD" => Some(Self::Upload),
            "DOWNLOAD" => Some(Self::Download),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Canceled,
}

impl TransferStatus {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Queued => "QUEUED",
            Self::Running => "RUNNING",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
            Self::Canceled => "CANCELED",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        [
            Self::Queued,
            Self::Running,
            Self::Completed,
            Self::Failed,
            Self::Canceled,
        ]
        .into_iter()
        .find(|s| s.as_db_str() == value)
    }
}

/// A persisted transfer. `local_path` is privacy-sensitive and never sent to
/// the frontend; only its file name is (DB design §9).
#[derive(Debug, Clone)]
pub struct StoredTransfer {
    pub id: String,
    pub connection_id: Option<String>,
    pub connection_name_snapshot: String,
    pub direction: TransferDirection,
    pub bucket: String,
    pub object_key: String,
    pub local_path: String,
    pub bytes_total: Option<i64>,
    pub bytes_transferred: i64,
    pub status: TransferStatus,
    pub retry_count: i64,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub multipart_upload_id: Option<String>,
    pub overwrite: bool,
    /// Download only: ETag of the object the temporary file belongs to.
    pub resume_etag: Option<String>,
    /// Download only: bytes of the temporary file known to be on disk.
    pub resume_offset: i64,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TransferDto {
    pub id: String,
    pub connection_id: Option<String>,
    pub connection_name_snapshot: String,
    pub direction: TransferDirection,
    pub bucket: String,
    pub object_key: String,
    /// Base name of the local file only.
    pub local_file_name: String,
    #[specta(type = Option<f64>)]
    pub bytes_total: Option<i64>,
    #[specta(type = f64)]
    pub bytes_transferred: i64,
    pub status: TransferStatus,
    #[specta(type = u32)]
    pub retry_count: i64,
    pub error: Option<AppError>,
    /// A failed download that can continue from this many bytes.
    #[specta(type = Option<f64>)]
    pub resumable_bytes: Option<i64>,
    /// Stopped by the user with Pause (status is FAILED with code PAUSED).
    pub paused: bool,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

impl From<&StoredTransfer> for TransferDto {
    fn from(t: &StoredTransfer) -> Self {
        let local_file_name = std::path::Path::new(&t.local_path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            id: t.id.clone(),
            connection_id: t.connection_id.clone(),
            connection_name_snapshot: t.connection_name_snapshot.clone(),
            direction: t.direction,
            bucket: t.bucket.clone(),
            object_key: t.object_key.clone(),
            local_file_name,
            bytes_total: t.bytes_total,
            bytes_transferred: t.bytes_transferred,
            status: t.status,
            retry_count: t.retry_count,
            error: t
                .error_message
                .as_ref()
                .map(|message| crate::errors::AppError::stored(t.error_code.as_deref(), message)),
            resumable_bytes: (t.direction == TransferDirection::Download
                && t.status == TransferStatus::Failed
                && t.resume_offset > 0)
                .then_some(t.resume_offset),
            paused: t.status == TransferStatus::Failed
                && t.error_code.as_deref() == Some(crate::transfers::PAUSED_CODE),
            created_at: t.created_at.clone(),
            started_at: t.started_at.clone(),
            completed_at: t.completed_at.clone(),
        }
    }
}

/// What to do when a transfer's destination already exists (TSD §6.5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferConflictPolicy {
    Ask,
    Overwrite,
    Skip,
    Rename,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueUploadInput {
    pub connection_id: String,
    pub bucket: String,
    pub prefix: String,
    pub selection_ids: Vec<String>,
    pub conflict_policy: TransferConflictPolicy,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueDownloadInput {
    pub connection_id: String,
    pub bucket: String,
    pub keys: Vec<String>,
    pub directory_selection_id: String,
    pub conflict_policy: TransferConflictPolicy,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EnqueueItemResult {
    #[serde(rename_all = "camelCase")]
    Queued { transfer_id: String, key: String },
    /// Not queued: the destination exists and the policy was `ASK`.
    /// `item` is the selection ID (upload) or object key (download) to resend.
    #[serde(rename_all = "camelCase")]
    Conflict { item: String, name: String },
    #[serde(rename_all = "camelCase")]
    Skipped { item: String, name: String },
    #[serde(rename_all = "camelCase")]
    Rejected {
        item: String,
        name: String,
        error: AppError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn download(status: TransferStatus, code: Option<&str>, offset: i64) -> StoredTransfer {
        StoredTransfer {
            id: "t1".into(),
            connection_id: Some("c1".into()),
            connection_name_snapshot: "S3".into(),
            direction: TransferDirection::Download,
            bucket: "b".into(),
            object_key: "video.mov".into(),
            local_path: "/Users/me/Downloads/video.mov".into(),
            bytes_total: Some(1000),
            bytes_transferred: offset,
            status,
            retry_count: 0,
            error_code: code.map(str::to_owned),
            error_message: code.map(|_| "x".to_owned()),
            multipart_upload_id: None,
            overwrite: false,
            resume_etag: Some("\"e\"".into()),
            resume_offset: offset,
            created_at: String::new(),
            started_at: None,
            completed_at: None,
        }
    }

    #[test]
    fn paused_and_resumable_downloads_are_reported() {
        let paused = TransferDto::from(&download(TransferStatus::Failed, Some("PAUSED"), 400));
        assert!(paused.paused);
        assert_eq!(paused.resumable_bytes, Some(400));
        assert_eq!(paused.local_file_name, "video.mov", "never the full path");

        let interrupted =
            TransferDto::from(&download(TransferStatus::Failed, Some("INTERRUPTED"), 400));
        assert!(!interrupted.paused);
        assert_eq!(interrupted.resumable_bytes, Some(400));

        let canceled = TransferDto::from(&download(
            TransferStatus::Canceled,
            Some("TRANSFER_CANCELED"),
            0,
        ));
        assert!(!canceled.paused);
        assert_eq!(canceled.resumable_bytes, None);

        // Paused before the first checkpoint: paused, but nothing to keep yet.
        let early = TransferDto::from(&download(TransferStatus::Failed, Some("PAUSED"), 0));
        assert!(early.paused);
        assert_eq!(early.resumable_bytes, None);
    }
}
