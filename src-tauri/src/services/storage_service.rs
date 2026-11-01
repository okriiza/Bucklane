use std::time::Duration;

use chrono::{SecondsFormat, Utc};

use crate::domain::bucket::{
    AddBucketBookmarkInput, BucketBookmark, BucketInfo, PublicUrl, SetBucketPublicUrlInput,
    UpdateBucketBookmarkInput, normalize_public_base_url, validate_bucket_name,
};
use crate::domain::connection::ProviderType;
use crate::errors::{AppError, ErrorCode};
use crate::providers::s3_provider::{S3StorageProvider, encode_key};
use crate::providers::storage_provider::StorageProvider;
use crate::repositories::bucket_bookmark_repository::BucketBookmarkRepository;
use crate::repositories::bucket_public_url_repository::BucketPublicUrlRepository;
use crate::services::connection_service::ConnectionService;
use crate::services::object_service::validate_key;

const MAX_DISPLAY_NAME_LEN: usize = 100;

/// Bucket listing, per-connection bucket bookmarks (BKT-001, BKT-005), and
/// public object URLs.
pub struct StorageService<'a> {
    connections: ConnectionService<'a>,
    bookmarks: BucketBookmarkRepository<'a>,
    public_urls: BucketPublicUrlRepository<'a>,
}

impl<'a> StorageService<'a> {
    pub fn new(
        connections: ConnectionService<'a>,
        bookmarks: BucketBookmarkRepository<'a>,
        public_urls: BucketPublicUrlRepository<'a>,
    ) -> Self {
        Self {
            connections,
            bookmarks,
            public_urls,
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

    /// Address of an object without a signature: the bucket's base URL if set,
    /// otherwise the provider's own address for it. Whether it works depends
    /// on the object being public, which is not checked.
    pub async fn public_url(
        &self,
        connection_id: &str,
        bucket: &str,
        key: &str,
    ) -> Result<PublicUrl, AppError> {
        let bucket = validate_bucket_name(bucket)?;
        validate_key(key)?;
        if key.ends_with('/') {
            return Err(AppError::new(
                ErrorCode::OperationUnsupported,
                "Folders have no URL; copy the URL of a file instead.",
            ));
        }
        let connection = self.connections.get(connection_id).await?;

        if let Some(base_url) = self.public_urls.get(connection_id, bucket).await? {
            let url = format!("{base_url}/{}", encode_key(key));
            return Ok(PublicUrl {
                url: Some(url),
                base_url: Some(base_url),
            });
        }
        if connection.provider == ProviderType::CloudflareR2 {
            return Ok(PublicUrl {
                url: None,
                base_url: None,
            });
        }

        // Presigning (local, no request) yields the exact address the SDK
        // uses: path or virtual-hosted style, and the bucket's region on AWS.
        let client = self
            .connections
            .client_for_bucket(connection_id, bucket)
            .await?;
        let signed = S3StorageProvider::new(client)
            .presign_get(bucket, key, Duration::from_secs(60))
            .await?;
        Ok(PublicUrl {
            url: Some(unsigned_url(connection.provider, &signed)),
            base_url: None,
        })
    }

    pub async fn set_public_base_url(
        &self,
        input: SetBucketPublicUrlInput,
    ) -> Result<Option<String>, AppError> {
        let bucket = validate_bucket_name(&input.bucket)?;
        self.connections.get(&input.connection_id).await?;
        let Some(value) = input.base_url.as_deref().filter(|v| !v.trim().is_empty()) else {
            self.public_urls
                .delete(&input.connection_id, bucket)
                .await?;
            return Ok(None);
        };
        let base_url = normalize_public_base_url(value)?;
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        self.public_urls
            .set(&input.connection_id, bucket, &base_url, &now)
            .await?;
        Ok(Some(base_url))
    }
}

/// Strips the signature from a presigned URL. Supabase serves public objects
/// from its own path next to the S3 API rather than from the S3 API itself.
fn unsigned_url(provider: ProviderType, presigned: &str) -> String {
    let url = presigned.split_once('?').map_or(presigned, |(url, _)| url);
    if provider == ProviderType::Supabase
        && let Some((origin, object)) = url.split_once("/storage/v1/s3/")
    {
        return format!("{origin}/storage/v1/object/public/{object}");
    }
    url.to_owned()
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
                BucketPublicUrlRepository::new(&self.pool),
            )
        }

        async fn connection(&self, provider: ProviderType, endpoint: &str) -> String {
            ConnectionService::new(&self.pool, &self.store, &self.clients)
                .create(ConnectionConfigInput {
                    name: "Other".into(),
                    provider,
                    endpoint: Some(endpoint.into()),
                    region: "us-east-1".into(),
                    access_key_id: "key".into(),
                    secret_access_key: Some(SecretString::new("secret")),
                    force_path_style: true,
                    allow_insecure_http: true,
                })
                .await
                .unwrap()
                .id
        }

        fn set_base(&self, connection_id: &str, base_url: Option<&str>) -> SetBucketPublicUrlInput {
            SetBucketPublicUrlInput {
                connection_id: connection_id.into(),
                bucket: "photos".into(),
                base_url: base_url.map(str::to_owned),
            }
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

    #[tokio::test]
    async fn public_url_prefers_the_bucket_base_url() {
        let f = Fixture::new().await;
        let id = f
            .connection(ProviderType::Minio, "http://localhost:9000")
            .await;
        let key = "cats/a b.jpg";

        let direct = f.service().public_url(&id, "photos", key).await.unwrap();
        assert_eq!(
            direct.url.as_deref(),
            Some("http://localhost:9000/photos/cats/a%20b.jpg")
        );
        assert_eq!(direct.base_url, None);

        let saved = f
            .service()
            .set_public_base_url(f.set_base(&id, Some("https://cdn.example.com/")))
            .await
            .unwrap();
        assert_eq!(saved.as_deref(), Some("https://cdn.example.com"));
        let custom = f.service().public_url(&id, "photos", key).await.unwrap();
        assert_eq!(
            custom.url.as_deref(),
            Some("https://cdn.example.com/cats/a%20b.jpg")
        );

        f.service()
            .set_public_base_url(f.set_base(&id, Some(" ")))
            .await
            .unwrap();
        let cleared = f.service().public_url(&id, "photos", key).await.unwrap();
        assert_eq!(cleared.url, direct.url);
    }

    #[tokio::test]
    async fn r2_has_no_public_url_without_a_base_url() {
        let f = Fixture::new().await;
        let id = f
            .connection(
                ProviderType::CloudflareR2,
                "https://acct.r2.cloudflarestorage.com",
            )
            .await;
        let url = f
            .service()
            .public_url(&id, "photos", "a.jpg")
            .await
            .unwrap();
        assert_eq!(url.url, None);

        assert!(
            f.service()
                .public_url(&id, "photos", "folder/")
                .await
                .is_err(),
            "folders have no URL"
        );
        assert!(
            f.service()
                .set_public_base_url(f.set_base(&id, Some("cdn.example.com")))
                .await
                .is_err()
        );
    }

    #[test]
    fn unsigned_url_drops_the_signature() {
        assert_eq!(
            unsigned_url(
                ProviderType::Aws,
                "https://b.s3.us-east-1.amazonaws.com/a%20b.jpg?X-Amz-Signature=abc"
            ),
            "https://b.s3.us-east-1.amazonaws.com/a%20b.jpg"
        );
        assert_eq!(
            unsigned_url(
                ProviderType::Supabase,
                "https://ref.supabase.co/storage/v1/s3/avatars/u/1.png?X-Amz-Signature=abc"
            ),
            "https://ref.supabase.co/storage/v1/object/public/avatars/u/1.png"
        );
    }
}
