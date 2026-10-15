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
