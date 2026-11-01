use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::errors::{AppError, ErrorCode};

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
    pub cache_control: Option<String>,
    pub content_disposition: Option<String>,
    pub content_encoding: Option<String>,
    pub content_language: Option<String>,
    /// Not necessarily an MD5 of the content (TSD §9.6).
    pub etag: Option<String>,
    pub storage_class: Option<String>,
    /// User-defined `x-amz-meta-*` headers.
    pub metadata: BTreeMap<String, String>,
}

/// AWS limit for the combined size of user metadata names and values.
const MAX_USER_METADATA_BYTES: usize = 2048;
const MAX_HEADER_VALUE_LEN: usize = 1024;

/// The editable headers of an object: standard HTTP headers and user metadata.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ObjectHeaders {
    pub content_type: Option<String>,
    pub cache_control: Option<String>,
    pub content_disposition: Option<String>,
    pub content_encoding: Option<String>,
    pub content_language: Option<String>,
    /// User-defined `x-amz-meta-*` headers, named without the prefix.
    pub metadata: BTreeMap<String, String>,
}

impl ObjectHeaders {
    /// Trims values, drops empty headers, and lowercases metadata names the
    /// way S3 stores them. Rejects anything that is not a valid HTTP header
    /// value: non-ASCII metadata is stored inconsistently across providers.
    pub fn normalized(self) -> Result<Self, AppError> {
        let header = |value: Option<String>, label: &str| {
            let Some(value) = value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()) else {
                return Ok(None);
            };
            if value.len() > MAX_HEADER_VALUE_LEN || !is_header_value(&value) {
                return Err(invalid_header(format!(
                    "{label} may only contain printable ASCII characters (at most {MAX_HEADER_VALUE_LEN})."
                )));
            }
            Ok(Some(value))
        };

        let mut metadata = BTreeMap::new();
        for (name, value) in self.metadata {
            let name = name.trim().to_ascii_lowercase();
            let name = name.strip_prefix("x-amz-meta-").unwrap_or(&name).to_owned();
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
            {
                return Err(invalid_header(
                    "Metadata names may only contain letters, digits, '-', '_' and '.'.",
                ));
            }
            let value = value.trim().to_owned();
            if !is_header_value(&value) {
                return Err(invalid_header(format!(
                    "The value of \"{name}\" may only contain printable ASCII characters."
                )));
            }
            if metadata.insert(name.clone(), value).is_some() {
                return Err(invalid_header(format!(
                    "The metadata name \"{name}\" is used more than once."
                )));
            }
        }
        let size: usize = metadata.iter().map(|(n, v)| n.len() + v.len()).sum();
        if size > MAX_USER_METADATA_BYTES {
            return Err(invalid_header(format!(
                "User metadata may be at most {MAX_USER_METADATA_BYTES} bytes in total."
            )));
        }

        Ok(Self {
            content_type: header(self.content_type, "Content-Type")?,
            cache_control: header(self.cache_control, "Cache-Control")?,
            content_disposition: header(self.content_disposition, "Content-Disposition")?,
            content_encoding: header(self.content_encoding, "Content-Encoding")?,
            content_language: header(self.content_language, "Content-Language")?,
            metadata,
        })
    }
}

fn is_header_value(value: &str) -> bool {
    value.bytes().all(|b| b == b' ' || b.is_ascii_graphic())
}

fn invalid_header(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::InvalidInput, message)
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateObjectMetadataInput {
    pub connection_id: String,
    pub bucket: String,
    pub key: String,
    /// ETag of the version the user edited; nothing changes if the object
    /// was replaced since.
    pub etag: String,
    pub headers: ObjectHeaders,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(metadata: &[(&str, &str)]) -> ObjectHeaders {
        ObjectHeaders {
            metadata: metadata
                .iter()
                .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                .collect(),
            ..ObjectHeaders::default()
        }
    }

    #[test]
    fn normalizes_headers_and_metadata_names() {
        let normalized = ObjectHeaders {
            content_type: Some("  text/plain ".into()),
            cache_control: Some("   ".into()),
            ..headers(&[("X-Amz-Meta-Owner", " dewi "), ("Build.ID", "42")])
        }
        .normalized()
        .unwrap();

        assert_eq!(normalized.content_type.as_deref(), Some("text/plain"));
        assert_eq!(normalized.cache_control, None, "blank headers are dropped");
        assert_eq!(normalized.metadata["owner"], "dewi");
        assert_eq!(normalized.metadata["build.id"], "42");
    }

    #[test]
    fn rejects_invalid_headers_and_metadata() {
        let bad_header = ObjectHeaders {
            content_disposition: Some("attachment\r\nX-Evil: 1".into()),
            ..ObjectHeaders::default()
        };
        assert!(bad_header.normalized().is_err(), "header injection");
        assert!(headers(&[("owner name", "x")]).normalized().is_err());
        assert!(headers(&[("x-amz-meta-", "x")]).normalized().is_err());
        assert!(
            headers(&[("owner", "Dédé")]).normalized().is_err(),
            "non-ASCII"
        );
        assert!(
            headers(&[("Owner", "a"), ("owner", "b")])
                .normalized()
                .is_err(),
            "duplicate after lowercasing"
        );
        let long = "x".repeat(MAX_USER_METADATA_BYTES);
        assert!(headers(&[("big", &long)]).normalized().is_err());
    }
}
