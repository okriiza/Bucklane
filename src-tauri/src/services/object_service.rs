use crate::domain::bucket::validate_bucket_name;
use crate::domain::object::{ListObjectsRequest, ListObjectsResponse, ObjectMetadata};
use crate::errors::{AppError, ErrorCode};
use crate::providers::s3_provider::S3StorageProvider;
use crate::providers::storage_provider::{ListQuery, StorageProvider};
use crate::services::connection_service::ConnectionService;

const DEFAULT_DELIMITER: &str = "/";
/// S3 keys are at most 1024 bytes; prefixes and continuation tokens are
/// bounded similarly to reject obviously malformed input early.
const MAX_KEY_LEN: usize = 1024;

/// Read-only object operations for the explorer (EXP-001, EXP-002, EXP-005).
pub struct ObjectService<'a> {
    connections: ConnectionService<'a>,
}

impl<'a> ObjectService<'a> {
    pub fn new(connections: ConnectionService<'a>) -> Self {
        Self { connections }
    }

    /// One page of a folder-style listing. Never lists a whole bucket at once.
    pub async fn list_objects(
        &self,
        request: ListObjectsRequest,
    ) -> Result<ListObjectsResponse, AppError> {
        let bucket = validate_bucket_name(&request.bucket)?;
        let prefix = request.prefix.as_deref().unwrap_or("");
        check_len(prefix, "prefix")?;
        let delimiter = request.delimiter.as_deref().unwrap_or(DEFAULT_DELIMITER);

        let client = self
            .connections
            .client_for_bucket(&request.connection_id, bucket)
            .await?;
        S3StorageProvider::new(client)
            .list_objects(ListQuery {
                bucket,
                prefix,
                delimiter,
                continuation_token: request.continuation_token.as_deref(),
                page_size: request.page_size,
            })
            .await
    }

    pub async fn head_object(
        &self,
        connection_id: &str,
        bucket: &str,
        key: &str,
    ) -> Result<ObjectMetadata, AppError> {
        let bucket = validate_bucket_name(bucket)?;
        if key.is_empty() {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "The object key is empty.",
            ));
        }
        check_len(key, "object key")?;

        let client = self
            .connections
            .client_for_bucket(connection_id, bucket)
            .await?;
        S3StorageProvider::new(client)
            .head_object(bucket, key)
            .await
    }
}

fn check_len(value: &str, what: &str) -> Result<(), AppError> {
    if value.len() > MAX_KEY_LEN {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            format!("The {what} is longer than {MAX_KEY_LEN} bytes."),
        ));
    }
    Ok(())
}
