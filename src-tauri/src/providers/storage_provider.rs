use std::time::Duration;

use async_trait::async_trait;

use crate::domain::bucket::BucketInfo;
use crate::domain::object::{
    DeleteObjectsResult, ListObjectsResponse, ObjectHeaders, ObjectMetadata,
};
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

/// The object a server-side copy reads.
#[derive(Debug, Clone, Copy)]
pub struct CopySource<'a> {
    pub bucket: &'a str,
    pub key: &'a str,
    /// The copy fails if the source no longer has this ETag.
    pub etag: Option<&'a str>,
    /// Decides between a single copy and a multipart copy.
    pub size: i64,
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

    /// Server-side copy to `bucket`/`key` on the same connection. Objects
    /// over 5 GiB, the limit of a single copy, are copied in parts.
    async fn copy_object(
        &self,
        source: CopySource<'_>,
        bucket: &str,
        key: &str,
    ) -> Result<(), AppError>;

    /// Replaces the object's headers and user metadata with a server-side
    /// copy onto itself; the content is unchanged. Fails if the object no
    /// longer has `etag`.
    async fn replace_metadata(
        &self,
        bucket: &str,
        key: &str,
        etag: &str,
        headers: &ObjectHeaders,
    ) -> Result<(), AppError>;

    async fn delete_object(&self, bucket: &str, key: &str) -> Result<(), AppError>;

    /// At most `limit` bytes from the start of the object, and whether it
    /// continues past them. Reading stops at the cap even if the server
    /// ignores the HTTP Range, so a large object never lands in memory.
    async fn get_bytes(
        &self,
        bucket: &str,
        key: &str,
        limit: u64,
    ) -> Result<(Vec<u8>, bool), AppError>;

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
