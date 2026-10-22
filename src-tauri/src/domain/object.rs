use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

/// An entry in a listing: either an object or a virtual folder (common prefix).
///
/// `key` is an opaque S3 identifier, never a local filesystem path (TSD §4.2).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StorageObject {
    pub key: String,
    /// Display name relative to the listed prefix; folders keep a trailing `/`.
    pub name: String,
    /// Sizes stay well below 2^53, so a JS number is lossless.
    #[specta(type = Option<f64>)]
    pub size: Option<i64>,
    pub last_modified: Option<String>,
    pub etag: Option<String>,
    pub storage_class: Option<String>,
    pub is_prefix: bool,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ListObjectsRequest {
    pub connection_id: String,
    pub bucket: String,
    pub prefix: Option<String>,
    /// Defaults to `/` for folder-style navigation.
    pub delimiter: Option<String>,
    /// Opaque; passed back exactly as received.
    pub continuation_token: Option<String>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ListObjectsResponse {
    /// Folders first, then objects, each in the provider's key order.
    pub objects: Vec<StorageObject>,
    pub next_continuation_token: Option<String>,
    pub is_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ObjectMetadata {
    pub key: String,
    #[specta(type = Option<f64>)]
    pub size: Option<i64>,
    pub last_modified: Option<String>,
    pub content_type: Option<String>,
    /// Not necessarily an MD5 of the content (TSD §9.6).
    pub etag: Option<String>,
    pub storage_class: Option<String>,
    /// User-defined `x-amz-meta-*` headers.
    pub metadata: BTreeMap<String, String>,
}

/// What to do when the destination key already exists (ADR-013).
/// `SKIP` and `RENAME` are added with transfers in M04.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConflictPolicy {
    /// Do nothing and report the conflict so the user can decide.
    Ask,
    Overwrite,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateFolderInput {
    pub connection_id: String,
    pub bucket: String,
    /// Parent folder prefix; empty for the bucket root.
    pub prefix: String,
    pub name: String,
}

/// Copy, move, or rename (a move within the same folder) of a single object.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CopyObjectInput {
    pub connection_id: String,
    pub bucket: String,
    pub source_key: String,
    pub destination_key: String,
    pub conflict_policy: ConflictPolicy,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ObjectOperationOutcome {
    Completed,
    /// Nothing was changed because the destination exists.
    #[serde(rename_all = "camelCase")]
    Conflict {
        #[specta(type = Option<f64>)]
        existing_size: Option<i64>,
        existing_last_modified: Option<String>,
    },
    /// Move only: the copy succeeded but the source was deliberately kept.
    #[serde(rename_all = "camelCase")]
    SourceKept {
        reason: String,
    },
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DeleteObjectsInput {
    pub connection_id: String,
    pub bucket: String,
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFailure {
    pub key: String,
    pub error: crate::errors::AppError,
}

/// Partial failure is normal; the UI must never report full success when
/// `failed` is non-empty (FIL-006).
#[derive(Debug, Clone, Default, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DeleteObjectsResult {
    pub deleted: Vec<String>,
    pub failed: Vec<DeleteFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelocateMode {
    Copy,
    Move,
}

/// What a folder copy/move does with objects that already exist at the
/// destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExistingObjectPolicy {
    Overwrite,
    Skip,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PrefixRelocateInput {
    pub connection_id: String,
    pub bucket: String,
    /// Folder to copy or move, ending in `/`.
    pub source_prefix: String,
    /// The folder's new full path, ending in `/` (e.g. `archive/photos/`).
    pub destination_prefix: String,
    pub mode: RelocateMode,
}

/// Snapshot of a folder copy/move for the user to confirm.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PrefixRelocatePlan {
    pub plan_id: String,
    pub mode: RelocateMode,
    pub source_prefix: String,
    pub destination_prefix: String,
    pub object_count: u32,
    #[specta(type = f64)]
    pub total_bytes: i64,
    /// Objects that already exist at the destination.
    pub conflict_count: u32,
    pub conflict_samples: Vec<String>,
    pub expires_at: String,
}

/// Outcome of a folder copy/move. Partial results are normal and must be
/// reported as such.
#[derive(Debug, Clone, Default, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PrefixRelocateResult {
    pub completed: u32,
    /// Existing destinations left alone under the Skip policy.
    pub skipped: u32,
    /// Objects not started because the user canceled.
    pub canceled: u32,
    pub failed: Vec<DeleteFailure>,
    /// Move only: copied, but the original was kept (it changed, or could
    /// not be deleted).
    pub sources_kept: Vec<DeleteFailure>,
}

/// Snapshot of the keys a folder delete will remove (TSD §6.3.2).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PrefixDeletePlan {
    pub plan_id: String,
    pub bucket: String,
    pub prefix: String,
    pub object_count: u32,
    #[specta(type = f64)]
    pub total_bytes: i64,
    pub sample_keys: Vec<String>,
    pub expires_at: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateShareLinkInput {
    pub connection_id: String,
    pub bucket: String,
    pub key: String,
    /// 60 seconds to 7 days (the SigV4 maximum).
    pub expires_in_seconds: u32,
}

/// A presigned download URL. Never persisted or logged (SEC-004); it grants
/// access to anyone holding it until `expires_at`.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ShareLink {
    pub url: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PreviewKind {
    Image,
    Pdf,
    Text,
}

/// Object contents for in-app preview, fetched by the backend so the
/// webview's CSP never has to allow remote origins.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ObjectPreview {
    pub kind: PreviewKind,
    pub content_type: String,
    pub data_base64: String,
    /// Text previews include only the beginning of large files.
    pub truncated: bool,
    #[specta(type = Option<f64>)]
    pub size: Option<i64>,
}
