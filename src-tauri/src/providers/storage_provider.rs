use std::time::Duration;

use async_trait::async_trait;

use crate::domain::bucket::BucketInfo;
use crate::domain::object::{DeleteObjectsResult, ListObjectsResponse, ObjectMetadata};
use crate::errors::AppError;

/// One page of a ListObjectsV2 request.
pub struct ListQuery<'a> {
    pub bucket: &'a str,
    pub prefix: &'a str,
    /// `None` lists every key under the prefix recursively.
    pub delimiter: Option<&'a str>,
    pub continuation_token: Option<&'a str>,
    pub page_size: Option<u32>,
}

/// Provider-agnostic storage operations (TSD §5.1). Streaming and multipart
/// operations are added with the transfer engine.
#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn list_buckets(&self) -> Result<Vec<BucketInfo>, AppError>;

    async fn head_bucket(&self, bucket: &str) -> Result<(), AppError>;

    async fn list_objects(&self, query: ListQuery<'_>) -> Result<ListObjectsResponse, AppError>;

    async fn head_object(&self, bucket: &str, key: &str) -> Result<ObjectMetadata, AppError>;

    /// Creates an empty object; used for folder markers.
    async fn put_empty_object(&self, bucket: &str, key: &str) -> Result<(), AppError>;

    /// Server-side copy within a bucket. With `if_match`, the copy fails if
    /// the source changed since its ETag was read.
    async fn copy_object(
        &self,
        bucket: &str,
        source_key: &str,
        destination_key: &str,
        if_match: Option<&str>,
    ) -> Result<(), AppError>;

    async fn delete_object(&self, bucket: &str, key: &str) -> Result<(), AppError>;

    /// Object bytes, or only the first `max_bytes` when given (HTTP Range).
    async fn get_bytes(
        &self,
        bucket: &str,
        key: &str,
        max_bytes: Option<u64>,
    ) -> Result<Vec<u8>, AppError>;

    /// Presigned GET URL valid for `expires_in`. Signing is local; no request
    /// is sent to the provider.
    async fn presign_get(
        &self,
        bucket: &str,
        key: &str,
        expires_in: Duration,
    ) -> Result<String, AppError>;

    /// Deletes up to 1000 keys, reporting per-key failures.
    async fn delete_objects(
        &self,
        bucket: &str,
        keys: &[String],
    ) -> Result<DeleteObjectsResult, AppError>;
}
