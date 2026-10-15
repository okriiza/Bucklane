use chrono::{SecondsFormat, Utc};

use crate::domain::bucket::{
    AddBucketBookmarkInput, BucketBookmark, BucketInfo, UpdateBucketBookmarkInput,
    validate_bucket_name,
};
use crate::errors::{AppError, ErrorCode};
use crate::providers::s3_provider::S3StorageProvider;
use crate::providers::storage_provider::StorageProvider;
use crate::repositories::bucket_bookmark_repository::BucketBookmarkRepository;
use crate::services::connection_service::ConnectionService;

const MAX_DISPLAY_NAME_LEN: usize = 100;

/// Bucket listing and per-connection bucket bookmarks (BKT-001, BKT-005).
pub struct StorageService<'a> {
    connections: ConnectionService<'a>,
    bookmarks: BucketBookmarkRepository<'a>,
}

impl<'a> StorageService<'a> {
    pub fn new(
        connections: ConnectionService<'a>,
        bookmarks: BucketBookmarkRepository<'a>,
    ) -> Self {
        Self {
            connections,
            bookmarks,
        }
    }

    pub async fn list_buckets(&self, connection_id: &str) -> Result<Vec<BucketInfo>, AppError> {
        let client = self.connections.client(connection_id).await?;
        let mut buckets = S3StorageProvider::new(client).list_buckets().await?;
        buckets.sort_by_key(|b| b.name.to_lowercase());
        Ok(buckets)
    }

    pub async fn head_bucket(&self, connection_id: &str, bucket: &str) -> Result<(), AppError> {
        let bucket = validate_bucket_name(bucket)?;
        let client = self
            .connections
            .client_for_bucket(connection_id, bucket)
            .await?;
        S3StorageProvider::new(client).head_bucket(bucket).await
    }

    pub async fn list_bookmarks(
        &self,
        connection_id: &str,
    ) -> Result<Vec<BucketBookmark>, AppError> {
        self.bookmarks.list(connection_id).await
    }

    /// Idempotent: bookmarking an already saved bucket returns the existing
    /// bookmark. Remote access is not checked; it is verified when opened.
    pub async fn add_bookmark(
        &self,
        input: AddBucketBookmarkInput,
    ) -> Result<BucketBookmark, AppError> {
        // Fails with a clear error if the connection was deleted meanwhile.
        self.connections.get(&input.connection_id).await?;
        let bookmark = BucketBookmark {
            id: uuid::Uuid::now_v7().to_string(),
            connection_id: input.connection_id,
            bucket_name: validate_bucket_name(&input.bucket_name)?.to_owned(),
            display_name: normalize_display_name(input.display_name.as_deref())?,
            created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        };

        if self.bookmarks.insert(&bookmark).await? {
            return Ok(bookmark);
        }
        self.bookmarks
            .list(&bookmark.connection_id)
            .await?
            .into_iter()
            .find(|b| b.bucket_name == bookmark.bucket_name)
            .ok_or_else(|| AppError::internal("Bookmark disappeared while saving."))
    }

    pub async fn update_bookmark(
        &self,
        input: UpdateBucketBookmarkInput,
    ) -> Result<BucketBookmark, AppError> {
        let display_name = normalize_display_name(input.display_name.as_deref())?;
        if !self
            .bookmarks
            .update_display_name(&input.id, display_name.as_deref())
            .await?
        {
            return Err(bookmark_not_found());
        }
        self.bookmarks
            .get(&input.id)
            .await?
            .ok_or_else(bookmark_not_found)
    }

    pub async fn remove_bookmark(&self, id: &str) -> Result<(), AppError> {
        if self.bookmarks.delete(id).await? {
            Ok(())
        } else {
            Err(bookmark_not_found())
        }
    }
}

fn normalize_display_name(name: Option<&str>) -> Result<Option<String>, AppError> {
    let name = name.map(str::trim).filter(|n| !n.is_empty());
    if name.is_some_and(|n| n.chars().count() > MAX_DISPLAY_NAME_LEN) {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            format!("The display name must be at most {MAX_DISPLAY_NAME_LEN} characters."),
        ));
    }
    Ok(name.map(str::to_owned))
}

fn bookmark_not_found() -> AppError {
    AppError::new(ErrorCode::InvalidInput, "The bookmark no longer exists.")
}

#[cfg(test)]
mod tests {
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
        connection_id: String,
    }

    impl Fixture {
        async fn new() -> Self {
            let pool = test_pool().await;
            let store = MemoryCredentialStore::default();
            let clients = ClientCache::default();
            let connection_id = ConnectionService::new(&pool, &store, &clients)
                .create(ConnectionConfigInput {
                    name: "AWS".into(),
                    provider: ProviderType::Aws,
                    endpoint: None,
                    region: "us-east-1".into(),
                    access_key_id: "AKIA".into(),
                    secret_access_key: Some(SecretString::new("secret")),
                    force_path_style: false,
                    allow_insecure_http: false,
                })
                .await
                .unwrap()
                .id;
            Self {
                pool,
                store,
                clients,
                connection_id,
            }
        }

        fn service(&self) -> StorageService<'_> {
            StorageService::new(
                ConnectionService::new(&self.pool, &self.store, &self.clients),
                BucketBookmarkRepository::new(&self.pool),
            )
        }

        fn add(&self, bucket: &str) -> AddBucketBookmarkInput {
            AddBucketBookmarkInput {
                connection_id: self.connection_id.clone(),
                bucket_name: bucket.into(),
                display_name: Some("  ".into()),
            }
        }
    }

    #[tokio::test]
    async fn adding_the_same_bookmark_twice_returns_the_existing_one() {
        let f = Fixture::new().await;
        let first = f.service().add_bookmark(f.add(" photos ")).await.unwrap();
        let second = f.service().add_bookmark(f.add("photos")).await.unwrap();

        assert_eq!(first.bucket_name, "photos");
        assert_eq!(first.display_name, None, "blank display name is dropped");
        assert_eq!(first.id, second.id);
        assert_eq!(
            f.service()
                .list_bookmarks(&f.connection_id)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn rejects_invalid_bucket_names_and_unknown_connections() {
        let f = Fixture::new().await;
        for bad in ["", "a/b", "has space"] {
            let err = f.service().add_bookmark(f.add(bad)).await.unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidInput, "{bad:?}");
        }

        let mut orphan = f.add("photos");
        orphan.connection_id = "missing".into();
        assert!(f.service().add_bookmark(orphan).await.is_err());
    }

    #[tokio::test]
    async fn updates_and_removes_bookmarks() {
        let f = Fixture::new().await;
        let bookmark = f.service().add_bookmark(f.add("photos")).await.unwrap();

        let renamed = f
            .service()
            .update_bookmark(UpdateBucketBookmarkInput {
                id: bookmark.id.clone(),
                display_name: Some("Photos (prod)".into()),
            })
            .await
            .unwrap();
        assert_eq!(renamed.display_name.as_deref(), Some("Photos (prod)"));

        f.service().remove_bookmark(&bookmark.id).await.unwrap();
        assert!(
            f.service()
                .list_bookmarks(&f.connection_id)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(f.service().remove_bookmark(&bookmark.id).await.is_err());
    }
}
