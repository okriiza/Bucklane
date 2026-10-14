use async_trait::async_trait;

use crate::domain::bucket::BucketInfo;
use crate::domain::object::{ListObjectsResponse, ObjectMetadata};
use crate::errors::AppError;

/// One page of a ListObjectsV2 request.
pub struct ListQuery<'a> {
    pub bucket: &'a str,
    pub prefix: &'a str,
    pub delimiter: &'a str,
    pub continuation_token: Option<&'a str>,
    pub page_size: Option<u32>,
}

/// Provider-agnostic storage operations (TSD §5.1). Write and multipart
/// operations are added with the object-operation and transfer milestones.
#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn list_buckets(&self) -> Result<Vec<BucketInfo>, AppError>;

    async fn head_bucket(&self, bucket: &str) -> Result<(), AppError>;

    async fn list_objects(&self, query: ListQuery<'_>) -> Result<ListObjectsResponse, AppError>;

    async fn head_object(&self, bucket: &str, key: &str) -> Result<ObjectMetadata, AppError>;
}
