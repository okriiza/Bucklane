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
