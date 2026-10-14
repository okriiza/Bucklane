use async_trait::async_trait;

use crate::domain::bucket::BucketInfo;
use crate::errors::AppError;

/// Provider-agnostic storage operations (TSD §5.1). Object and multipart
/// operations are added with the explorer and transfer milestones.
#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn list_buckets(&self) -> Result<Vec<BucketInfo>, AppError>;

    async fn head_bucket(&self, bucket: &str) -> Result<(), AppError>;
}
