use serde::{Deserialize, Serialize};
use specta::Type;

use crate::errors::{AppError, ErrorCode};

/// Lenient check that accepts any provider's naming rules but rejects values
/// that cannot be a bucket name at all.
pub fn validate_bucket_name(bucket: &str) -> Result<&str, AppError> {
    let bucket = bucket.trim();
    if bucket.is_empty()
        || bucket.len() > 255
        || bucket.contains('/')
        || bucket.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            "The bucket name is not valid.",
        ));
    }
    Ok(bucket)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BucketInfo {
    pub name: String,
    pub created_at: Option<String>,
    /// Reported by AWS ListBuckets; other providers usually omit it.
    pub region: Option<String>,
}

/// A bucket the user saved by name so it can be opened without ListBuckets
/// permission (BKT-005). Not evidence that the bucket exists.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BucketBookmark {
    pub id: String,
    pub connection_id: String,
    pub bucket_name: String,
    pub display_name: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AddBucketBookmarkInput {
    pub connection_id: String,
    pub bucket_name: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBucketBookmarkInput {
    pub id: String,
    pub display_name: Option<String>,
}
