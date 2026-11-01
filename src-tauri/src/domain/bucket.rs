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

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetBucketPublicUrlInput {
    pub connection_id: String,
    pub bucket: String,
    /// `None` or blank removes the bucket's base URL.
    pub base_url: Option<String>,
}

/// Unsigned address of an object. It only works if the object is publicly
/// readable (bucket policy, public ACL, or a public custom domain).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PublicUrl {
    /// `None` when the bucket has no base URL and the provider's S3 endpoint
    /// never serves public reads (Cloudflare R2).
    pub url: Option<String>,
    /// The bucket's base URL (custom domain or CDN), if one is set.
    pub base_url: Option<String>,
}

const MAX_BASE_URL_LEN: usize = 500;

/// Validates a public base URL such as `https://cdn.example.com/assets`.
/// Returns it without a trailing slash, ready to have a key appended.
pub fn normalize_public_base_url(value: &str) -> Result<String, AppError> {
    let invalid = || {
        AppError::new(
            ErrorCode::InvalidInput,
            "Enter a base URL like https://cdn.example.com, without a query string.",
        )
    };
    let value = value.trim().trim_end_matches('/');
    if value.len() > MAX_BASE_URL_LEN {
        return Err(invalid());
    }
    let url = url::Url::parse(value).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_public_base_urls() {
        assert_eq!(
            normalize_public_base_url(" https://cdn.example.com/assets/ ").unwrap(),
            "https://cdn.example.com/assets"
        );
        assert_eq!(
            normalize_public_base_url("http://192.168.1.10:9000/media").unwrap(),
            "http://192.168.1.10:9000/media"
        );
        for bad in [
            "cdn.example.com",
            "ftp://cdn.example.com",
            "https://cdn.example.com/?v=1",
            "https://cdn.example.com/#top",
            "https://user:pass@cdn.example.com",
            "javascript:alert(1)",
        ] {
            assert!(normalize_public_base_url(bad).is_err(), "{bad}");
        }
    }
}
