use chrono::{SecondsFormat, Utc};

use crate::domain::bucket::validate_bucket_name;
use crate::domain::object::{
    ConflictPolicy, CopyObjectInput, CreateFolderInput, DeleteObjectsInput, DeleteObjectsResult,
    ListObjectsRequest, ListObjectsResponse, ObjectMetadata, ObjectOperationOutcome,
    PrefixDeletePlan,
};
use crate::errors::{AppError, ErrorCode};
use crate::providers::s3_provider::S3StorageProvider;
use crate::providers::storage_provider::{ListQuery, StorageProvider};
use crate::services::connection_service::ConnectionService;
use crate::services::delete_plans::{DeletePlanRegistry, PLAN_TTL};

const DEFAULT_DELIMITER: &str = "/";
/// S3 keys are at most 1024 bytes.
const MAX_KEY_LEN: usize = 1024;
/// Largest object a single CopyObject request may copy; larger objects need
/// multipart copy, which the MVP does not perform (TSD §9.6).
const MAX_SINGLE_COPY_BYTES: i64 = 5 * 1024 * 1024 * 1024;
const DELETE_BATCH: usize = 1000;
const MAX_PREFIX_DELETE_OBJECTS: usize = 100_000;
const SAMPLE_KEYS: usize = 10;

/// Object operations for the explorer (EXP-*, FIL-003..006).
pub struct ObjectService<'a> {
    connections: ConnectionService<'a>,
    delete_plans: &'a DeletePlanRegistry,
}

impl<'a> ObjectService<'a> {
    pub fn new(connections: ConnectionService<'a>, delete_plans: &'a DeletePlanRegistry) -> Self {
        Self {
            connections,
            delete_plans,
        }
    }

    async fn provider(
        &self,
        connection_id: &str,
        bucket: &str,
    ) -> Result<S3StorageProvider, AppError> {
        let client = self
            .connections
            .client_for_bucket(connection_id, bucket)
            .await?;
        Ok(S3StorageProvider::new(client))
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

        self.provider(&request.connection_id, bucket)
            .await?
            .list_objects(ListQuery {
                bucket,
                prefix,
                delimiter: Some(delimiter),
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
        validate_key(key)?;
        self.provider(connection_id, bucket)
            .await?
            .head_object(bucket, key)
            .await
    }

    /// Creates a zero-byte `name/` marker so an empty folder is visible
    /// (EXP-003). The marker is not a real directory.
    pub async fn create_folder(&self, input: CreateFolderInput) -> Result<String, AppError> {
        let bucket = validate_bucket_name(&input.bucket)?;
        let name = validate_folder_name(&input.name)?;
        if !input.prefix.is_empty() && !input.prefix.ends_with('/') {
            return Err(invalid("The parent folder is not valid."));
        }
        let key = format!("{}{name}/", input.prefix);
        validate_key(&key)?;

        let provider = self.provider(&input.connection_id, bucket).await?;
        // A folder exists if its marker exists or anything is stored under
        // it. Listings hide the listed folder's own marker, so check the
        // marker directly first.
        match provider.head_object(bucket, &key).await {
            Ok(_) => return Err(invalid("A folder with this name already exists.")),
            Err(err) if err.code == ErrorCode::ObjectNotFound => {}
            Err(err) => return Err(err),
        }
        let existing = provider
            .list_objects(ListQuery {
                bucket,
                prefix: &key,
                delimiter: Some(DEFAULT_DELIMITER),
                continuation_token: None,
                page_size: Some(1),
            })
            .await?;
        if !existing.objects.is_empty() || existing.is_truncated {
            return Err(invalid("A folder with this name already exists."));
        }

        provider.put_empty_object(bucket, &key).await?;
        tracing::info!(connection_id = %input.connection_id, "folder created");
        Ok(key)
    }

    pub async fn copy_object(
        &self,
        input: CopyObjectInput,
    ) -> Result<ObjectOperationOutcome, AppError> {
        let (provider, source) = self.prepare_copy(&input).await?;
        if let Some(conflict) = self.check_destination(&provider, &input).await? {
            return Ok(conflict);
        }
        copy_and_verify(&provider, &input, &source).await?;
        tracing::info!(connection_id = %input.connection_id, "object copied");
        Ok(ObjectOperationOutcome::Completed)
    }

    /// Safe move (TSD §9.6): copy with `If-Match` on the source ETag, verify
    /// the destination, re-check the source is unchanged, and only then
    /// delete it. Any doubt keeps the source.
    pub async fn move_object(
        &self,
        input: CopyObjectInput,
    ) -> Result<ObjectOperationOutcome, AppError> {
        let (provider, source) = self.prepare_copy(&input).await?;
        if let Some(conflict) = self.check_destination(&provider, &input).await? {
            return Ok(conflict);
        }
        copy_and_verify(&provider, &input, &source).await?;

        let bucket = input.bucket.trim();
        match provider.head_object(bucket, &input.source_key).await {
            Ok(current) if current.etag == source.etag => {}
            Ok(_) => {
                return Ok(source_kept(
                    "The original changed while it was being moved, so it was kept.",
                ));
            }
            Err(err) => return Ok(source_kept(&err.message)),
        }

        if let Err(err) = provider.delete_object(bucket, &input.source_key).await {
            return Ok(source_kept(&format!(
                "The copy succeeded, but the original could not be deleted: {}",
                err.message
            )));
        }
        tracing::info!(connection_id = %input.connection_id, "object moved");
        Ok(ObjectOperationOutcome::Completed)
    }

    pub async fn delete_objects(
        &self,
        input: DeleteObjectsInput,
    ) -> Result<DeleteObjectsResult, AppError> {
        let bucket = validate_bucket_name(&input.bucket)?;
        if input.keys.is_empty() {
            return Ok(DeleteObjectsResult::default());
        }
        for key in &input.keys {
            validate_key(key)?;
        }
        let provider = self.provider(&input.connection_id, bucket).await?;
        let result = delete_in_batches(&provider, bucket, &input.keys).await;
        tracing::info!(
            connection_id = %input.connection_id,
            deleted = result.deleted.len(),
            failed = result.failed.len(),
            "objects deleted"
        );
        Ok(result)
    }

    /// Lists every key under `prefix` and stores it as a single-use plan; the
    /// user confirms against the returned counts.
    pub async fn prepare_prefix_delete(
        &self,
        connection_id: &str,
        bucket: &str,
        prefix: &str,
    ) -> Result<PrefixDeletePlan, AppError> {
        let bucket = validate_bucket_name(bucket)?;
        if prefix.is_empty() {
            return Err(invalid(
                "Deleting an entire bucket's contents is not allowed.",
            ));
        }
        if !prefix.ends_with('/') {
            return Err(invalid("Only folders can be deleted this way."));
        }
        validate_key(prefix)?;

        let provider = self.provider(connection_id, bucket).await?;
        let mut keys = Vec::new();
        let mut total_bytes = 0i64;
        let mut token: Option<String> = None;
        loop {
            let page = provider
                .list_objects(ListQuery {
                    bucket,
                    prefix,
                    delimiter: None,
                    continuation_token: token.as_deref(),
                    page_size: Some(1000),
                })
                .await?;
            for object in page.objects {
                total_bytes += object.size.unwrap_or(0);
                keys.push(object.key);
            }
            if keys.len() > MAX_PREFIX_DELETE_OBJECTS {
                return Err(AppError::new(
                    ErrorCode::OperationUnsupported,
                    format!(
                        "This folder contains more than {MAX_PREFIX_DELETE_OBJECTS} objects, which is more than Bucklane can delete at once."
                    ),
                ));
            }
            if !page.is_truncated {
                break;
            }
            token = page.next_continuation_token;
        }
        // Listings hide the marker of the listed folder itself; include it
        // only if it really exists so the confirmed count stays exact.
        if !keys.iter().any(|k| k == prefix) {
            match provider.head_object(bucket, prefix).await {
                Ok(marker) => {
                    total_bytes += marker.size.unwrap_or(0);
                    keys.push(prefix.to_owned());
                }
                Err(err) if err.code == ErrorCode::ObjectNotFound => {}
                Err(err) => return Err(err),
            }
        }
        if keys.is_empty() {
            return Err(invalid("This folder is already empty."));
        }

        let sample_keys = keys.iter().take(SAMPLE_KEYS).cloned().collect();
        let object_count = u32::try_from(keys.len()).unwrap_or(u32::MAX);
        let expires_at = Utc::now() + PLAN_TTL;
        let plan_id = self.delete_plans.insert(connection_id, bucket, keys);

        Ok(PrefixDeletePlan {
            plan_id,
            bucket: bucket.to_owned(),
            prefix: prefix.to_owned(),
            object_count,
            total_bytes,
            sample_keys,
            expires_at: expires_at.to_rfc3339_opts(SecondsFormat::Millis, true),
        })
    }

    /// Deletes exactly the keys captured in the plan; objects added after the
    /// plan was created are left alone.
    pub async fn execute_prefix_delete(
        &self,
        plan_id: &str,
    ) -> Result<DeleteObjectsResult, AppError> {
        let plan = self.delete_plans.take(plan_id)?;
        let provider = self.provider(&plan.connection_id, &plan.bucket).await?;
        let result = delete_in_batches(&provider, &plan.bucket, &plan.keys).await;
        tracing::info!(
            connection_id = %plan.connection_id,
            deleted = result.deleted.len(),
            failed = result.failed.len(),
            "folder deleted"
        );
        Ok(result)
    }

    async fn prepare_copy(
        &self,
        input: &CopyObjectInput,
    ) -> Result<(S3StorageProvider, ObjectMetadata), AppError> {
        let bucket = validate_bucket_name(&input.bucket)?;
        validate_key(&input.source_key)?;
        validate_key(&input.destination_key)?;
        if input.source_key == input.destination_key {
            return Err(invalid("The destination is the same as the source."));
        }
        if input.source_key.ends_with('/') || input.destination_key.ends_with('/') {
            return Err(AppError::new(
                ErrorCode::OperationUnsupported,
                "Copying, moving, or renaming folders is not supported yet.",
            ));
        }

        let provider = self.provider(&input.connection_id, bucket).await?;
        let source = provider.head_object(bucket, &input.source_key).await?;
        if source.size.unwrap_or(0) > MAX_SINGLE_COPY_BYTES {
            return Err(AppError::new(
                ErrorCode::OperationUnsupported,
                "Objects larger than 5 GiB cannot be copied, moved, or renamed yet.",
            ));
        }
        Ok((provider, source))
    }

    async fn check_destination(
        &self,
        provider: &S3StorageProvider,
        input: &CopyObjectInput,
    ) -> Result<Option<ObjectOperationOutcome>, AppError> {
        match provider
            .head_object(input.bucket.trim(), &input.destination_key)
            .await
        {
            Ok(existing) if input.conflict_policy == ConflictPolicy::Ask => {
                Ok(Some(ObjectOperationOutcome::Conflict {
                    existing_size: existing.size,
                    existing_last_modified: existing.last_modified,
                }))
            }
            Ok(_) => Ok(None),
            Err(err) if err.code == ErrorCode::ObjectNotFound => Ok(None),
            Err(err) => Err(err),
        }
    }
}

async fn copy_and_verify(
    provider: &S3StorageProvider,
    input: &CopyObjectInput,
    source: &ObjectMetadata,
) -> Result<(), AppError> {
    let bucket = input.bucket.trim();
    provider
        .copy_object(
            bucket,
            &input.source_key,
            &input.destination_key,
            source.etag.as_deref(),
        )
        .await?;

    // ETags of copies are not guaranteed to match, so verify by size.
    let copied = provider.head_object(bucket, &input.destination_key).await?;
    if copied.size != source.size {
        return Err(AppError::new(
            ErrorCode::TransferFailed,
            "The copy could not be verified: its size differs from the original.",
        ));
    }
    Ok(())
}

/// Uses DeleteObjects in batches of 1000, falling back to one request per key
/// for providers that do not implement the batch API.
async fn delete_in_batches(
    provider: &S3StorageProvider,
    bucket: &str,
    keys: &[String],
) -> DeleteObjectsResult {
    let mut result = DeleteObjectsResult::default();
    for batch in keys.chunks(DELETE_BATCH) {
        match provider.delete_objects(bucket, batch).await {
            Ok(batch_result) => {
                result.deleted.extend(batch_result.deleted);
                result.failed.extend(batch_result.failed);
            }
            Err(err)
                if matches!(
                    err.code,
                    ErrorCode::OperationUnsupported | ErrorCode::InternalError
                ) =>
            {
                for key in batch {
                    match provider.delete_object(bucket, key).await {
                        Ok(()) => result.deleted.push(key.clone()),
                        Err(error) => result.failed.push(crate::domain::object::DeleteFailure {
                            key: key.clone(),
                            error,
                        }),
                    }
                }
            }
            Err(err) => {
                result.failed.extend(batch.iter().map(|key| {
                    crate::domain::object::DeleteFailure {
                        key: key.clone(),
                        error: err.clone(),
                    }
                }));
            }
        }
    }
    result
}

fn source_kept(reason: &str) -> ObjectOperationOutcome {
    ObjectOperationOutcome::SourceKept {
        reason: reason.to_owned(),
    }
}

fn validate_key(key: &str) -> Result<(), AppError> {
    if key.is_empty() {
        return Err(invalid("The object key is empty."));
    }
    check_len(key, "object key")
}

fn validate_folder_name(name: &str) -> Result<&str, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(invalid("Enter a folder name."));
    }
    if name.contains('/') || name == "." || name == ".." || name.chars().any(char::is_control) {
        return Err(invalid(
            "A folder name cannot contain \"/\" or be \".\" or \"..\".",
        ));
    }
    Ok(name)
}

fn check_len(value: &str, what: &str) -> Result<(), AppError> {
    if value.len() > MAX_KEY_LEN {
        return Err(invalid(format!(
            "The {what} is longer than {MAX_KEY_LEN} bytes."
        )));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_folder_names() {
        assert_eq!(validate_folder_name("  reports ").unwrap(), "reports");
        for bad in ["", "  ", "a/b", ".", "..", "tab\there"] {
            assert!(validate_folder_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn validates_key_length() {
        assert!(validate_key("").is_err());
        assert!(validate_key(&"k".repeat(1024)).is_ok());
        assert!(validate_key(&"k".repeat(1025)).is_err());
    }
}

/// Data-safety checks against a real S3-compatible server when
/// `BUCKLANE_S3_TEST_ENDPOINT` is set; skipped otherwise.
#[cfg(test)]
mod s3_integration_tests {
    use aws_sdk_s3::primitives::ByteStream;
    use sqlx::SqlitePool;

    use super::*;
    use crate::database::test_pool;
    use crate::domain::connection::{ConnectionConfigInput, ProviderType};
    use crate::providers::s3_client_factory::ClientCache;
    use crate::security::SecretString;
    use crate::security::credential_store::memory::MemoryCredentialStore;

    struct Fixture {
        pool: SqlitePool,
        store: MemoryCredentialStore,
        clients: ClientCache,
        plans: DeletePlanRegistry,
        connection_id: String,
        bucket: String,
    }

    impl Fixture {
        async fn new() -> Option<Self> {
            let Ok(endpoint) = std::env::var("BUCKLANE_S3_TEST_ENDPOINT") else {
                eprintln!("skipped: BUCKLANE_S3_TEST_ENDPOINT not set");
                return None;
            };
            let pool = test_pool().await;
            let store = MemoryCredentialStore::default();
            let clients = ClientCache::default();
            let connection_id = ConnectionService::new(&pool, &store, &clients)
                .create(ConnectionConfigInput {
                    name: "MinIO".into(),
                    provider: ProviderType::Minio,
                    endpoint: Some(endpoint),
                    region: "us-east-1".into(),
                    access_key_id: "bucklane-test".into(),
                    secret_access_key: Some(SecretString::new("bucklane-test-secret")),
                    force_path_style: true,
                    allow_insecure_http: true,
                })
                .await
                .unwrap()
                .id;
            let fixture = Self {
                pool,
                store,
                clients,
                plans: DeletePlanRegistry::default(),
                connection_id,
                bucket: format!("bucklane-ops-{}", uuid::Uuid::now_v7().simple()),
            };
            fixture
                .raw()
                .await
                .create_bucket()
                .bucket(&fixture.bucket)
                .send()
                .await
                .unwrap();
            Some(fixture)
        }

        fn connections(&self) -> ConnectionService<'_> {
            ConnectionService::new(&self.pool, &self.store, &self.clients)
        }

        fn service(&self) -> ObjectService<'_> {
            ObjectService::new(self.connections(), &self.plans)
        }

        async fn raw(&self) -> aws_sdk_s3::Client {
            self.connections()
                .client(&self.connection_id)
                .await
                .unwrap()
        }

        async fn put(&self, key: &str, body: &'static [u8]) {
            self.raw()
                .await
                .put_object()
                .bucket(&self.bucket)
                .key(key)
                .body(ByteStream::from_static(body))
                .send()
                .await
                .unwrap();
        }

        async fn exists(&self, key: &str) -> bool {
            self.service()
                .head_object(&self.connection_id, &self.bucket, key)
                .await
                .is_ok()
        }

        fn copy_input(&self, from: &str, to: &str, policy: ConflictPolicy) -> CopyObjectInput {
            CopyObjectInput {
                connection_id: self.connection_id.clone(),
                bucket: self.bucket.clone(),
                source_key: from.into(),
                destination_key: to.into(),
                conflict_policy: policy,
            }
        }
    }

    #[tokio::test]
    async fn create_folder_rejects_existing_folder() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        let input = CreateFolderInput {
            connection_id: f.connection_id.clone(),
            bucket: f.bucket.clone(),
            prefix: String::new(),
            name: "reports".into(),
        };

        assert_eq!(
            f.service().create_folder(input.clone()).await.unwrap(),
            "reports/"
        );
        assert!(f.exists("reports/").await);
        assert!(f.service().create_folder(input).await.is_err());
    }

    #[tokio::test]
    async fn copy_asks_before_overwriting() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        f.put("a.txt", b"original").await;
        f.put("b.txt", b"existing").await;

        let outcome = f
            .service()
            .copy_object(f.copy_input("a.txt", "b.txt", ConflictPolicy::Ask))
            .await
            .unwrap();
        assert!(matches!(
            outcome,
            ObjectOperationOutcome::Conflict {
                existing_size: Some(8),
                ..
            }
        ));

        let outcome = f
            .service()
            .copy_object(f.copy_input("a.txt", "b.txt", ConflictPolicy::Overwrite))
            .await
            .unwrap();
        assert!(matches!(outcome, ObjectOperationOutcome::Completed));
        assert!(f.exists("a.txt").await, "copy keeps the source");
    }

    #[tokio::test]
    async fn move_deletes_source_only_after_copy() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        f.put("docs/report draft.txt", b"content").await;

        let outcome = f
            .service()
            .move_object(f.copy_input(
                "docs/report draft.txt",
                "archive/report.txt",
                ConflictPolicy::Ask,
            ))
            .await
            .unwrap();

        assert!(matches!(outcome, ObjectOperationOutcome::Completed));
        assert!(f.exists("archive/report.txt").await);
        assert!(!f.exists("docs/report draft.txt").await);
    }

    #[tokio::test]
    async fn move_of_missing_source_changes_nothing() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        let err = f
            .service()
            .move_object(f.copy_input("missing.txt", "elsewhere.txt", ConflictPolicy::Ask))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::ObjectNotFound);
        assert!(!f.exists("elsewhere.txt").await);
    }

    #[tokio::test]
    async fn prefix_delete_removes_only_snapshotted_keys() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        for key in ["tmp/", "tmp/a.txt", "tmp/nested/b.txt", "keep.txt"] {
            f.put(key, b"x").await;
        }

        let plan = f
            .service()
            .prepare_prefix_delete(&f.connection_id, &f.bucket, "tmp/")
            .await
            .unwrap();
        assert_eq!(plan.object_count, 3);
        assert_eq!(plan.total_bytes, 3);

        // Added after confirmation: must survive.
        f.put("tmp/late.txt", b"x").await;

        let result = f
            .service()
            .execute_prefix_delete(&plan.plan_id)
            .await
            .unwrap();
        assert_eq!(result.deleted.len(), 3);
        assert!(result.failed.is_empty());
        assert!(f.exists("tmp/late.txt").await);
        assert!(f.exists("keep.txt").await);
        assert!(
            f.service()
                .execute_prefix_delete(&plan.plan_id)
                .await
                .is_err(),
            "single use"
        );
    }
}
