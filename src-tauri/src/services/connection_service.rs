use aws_sdk_s3::Client;
use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use specta::Type;
use sqlx::SqlitePool;

use crate::domain::bucket::validate_bucket_name;
use crate::domain::connection::{
    ConnectionConfigInput, ConnectionDto, ProviderType, ResolvedConfig, StorageConnection,
    TestConnectionInput, TestConnectionResult, TestMethod, UpdateConnectionInput,
};
use crate::errors::{AppError, ErrorCode};
use crate::providers::provider_profiles::profile;
use crate::providers::s3_client_factory::{ClientCache, build_client};
use crate::providers::s3_provider::S3StorageProvider;
use crate::providers::storage_provider::StorageProvider;
use crate::repositories::connection_repository::ConnectionRepository;
use crate::security::credential_store::{CredentialStore, new_reference};
use crate::security::endpoint_policy::resolve_endpoint;

const MAX_NAME_LEN: usize = 100;
const MAX_FIELD_LEN: usize = 256;

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DeleteConnectionResult {
    /// `false` if the connection was removed but its keychain entry could not
    /// be; the UI should warn the user (DB design §6.3).
    pub secret_removed: bool,
}

pub struct ConnectionService<'a> {
    pool: &'a SqlitePool,
    credentials: &'a dyn CredentialStore,
    clients: &'a ClientCache,
}

impl<'a> ConnectionService<'a> {
    pub fn new(
        pool: &'a SqlitePool,
        credentials: &'a dyn CredentialStore,
        clients: &'a ClientCache,
    ) -> Self {
        Self {
            pool,
            credentials,
            clients,
        }
    }

    fn repo(&self) -> ConnectionRepository<'a> {
        ConnectionRepository::new(self.pool)
    }

    pub async fn list(&self) -> Result<Vec<ConnectionDto>, AppError> {
        Ok(self
            .repo()
            .list()
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub async fn get(&self, id: &str) -> Result<ConnectionDto, AppError> {
        self.load(id).await.map(Into::into)
    }

    /// Stores the secret first, then the row; if the row insert fails the
    /// secret is removed again (DB design §6.1).
    pub async fn create(&self, input: ConnectionConfigInput) -> Result<ConnectionDto, AppError> {
        let name = validate_name(&input.name)?;
        let resolved = resolve_config(&input)?;
        let secret = input
            .secret_access_key
            .as_ref()
            .filter(|s| !s.is_blank())
            .ok_or_else(|| invalid("Enter the secret access key."))?;

        let now = now();
        let connection = StorageConnection {
            id: uuid::Uuid::now_v7().to_string(),
            name,
            provider: resolved.provider,
            endpoint: resolved.endpoint,
            region: resolved.region,
            access_key_id: resolved.access_key_id,
            credential_ref: new_reference(),
            force_path_style: resolved.force_path_style,
            allow_insecure_http: resolved.allow_insecure_http,
            created_at: now.clone(),
            updated_at: now,
        };

        self.credentials.save(&connection.credential_ref, secret)?;
        if let Err(err) = self.repo().insert(&connection).await {
            self.discard_secret(&connection.credential_ref);
            return Err(err);
        }

        tracing::info!(connection_id = %connection.id, provider = ?connection.provider, "connection created");
        Ok(connection.into())
    }

    /// A new secret is written under a new reference, the row is switched to
    /// it, and only then is the old secret deleted (TSD §8.2).
    pub async fn update(&self, input: UpdateConnectionInput) -> Result<ConnectionDto, AppError> {
        let existing = self.load(&input.id).await?;
        let config = input.config;
        let name = validate_name(&config.name)?;
        let resolved = resolve_config(&config)?;
        let new_secret = config.secret_access_key.as_ref().filter(|s| !s.is_blank());

        if resolved.access_key_id != existing.access_key_id && new_secret.is_none() {
            return Err(invalid(
                "Enter the secret access key for the new access key ID.",
            ));
        }

        let mut updated = StorageConnection {
            name,
            provider: resolved.provider,
            endpoint: resolved.endpoint,
            region: resolved.region,
            access_key_id: resolved.access_key_id,
            force_path_style: resolved.force_path_style,
            allow_insecure_http: resolved.allow_insecure_http,
            updated_at: now(),
            ..existing.clone()
        };

        if let Some(secret) = new_secret {
            updated.credential_ref = new_reference();
            self.credentials.save(&updated.credential_ref, secret)?;
        }

        let rotated = updated.credential_ref != existing.credential_ref;
        match self.repo().update(&updated).await {
            Ok(true) => {}
            outcome => {
                // The row still points at the old secret; drop the new one.
                if rotated {
                    self.discard_secret(&updated.credential_ref);
                }
                return Err(outcome.err().unwrap_or_else(not_found));
            }
        }

        self.clients.invalidate(&updated.id);
        if rotated {
            self.discard_secret(&existing.credential_ref);
        }

        tracing::info!(connection_id = %updated.id, rotated, "connection updated");
        Ok(updated.into())
    }

    // TODO(M04): refuse or cancel active transfers before deleting (TSD §7.2).
    pub async fn delete(&self, id: &str) -> Result<DeleteConnectionResult, AppError> {
        let existing = self.load(id).await?;
        if !self.repo().delete(id).await? {
            return Err(not_found());
        }
        self.clients.invalidate(id);

        let secret_removed = self.credentials.delete(&existing.credential_ref).is_ok();
        tracing::info!(connection_id = %id, secret_removed, "connection deleted");
        Ok(DeleteConnectionResult { secret_removed })
    }

    pub async fn test(&self, input: TestConnectionInput) -> Result<TestConnectionResult, AppError> {
        let test_bucket = input
            .test_bucket
            .as_deref()
            .filter(|b| !b.trim().is_empty())
            .map(validate_bucket_name)
            .transpose()?;

        let client = match (&input.config, &input.connection_id) {
            (Some(config), saved_id) => {
                self.client_for_unsaved(config, saved_id.as_deref()).await?
            }
            (None, Some(id)) => self.client_for_saved(&self.load(id).await?, None)?,
            (None, None) => return Err(invalid("Nothing to test.")),
        };
        let provider = S3StorageProvider::new(client);

        match provider.list_buckets().await {
            Ok(buckets) => Ok(TestConnectionResult {
                method: TestMethod::ListBuckets,
                bucket_count: Some(u32::try_from(buckets.len()).unwrap_or(u32::MAX)),
            }),
            // Permission to list buckets is optional (CON-002): fall back to
            // the bucket the user named, if any.
            Err(err) if err.code == ErrorCode::AccessDenied => match test_bucket {
                Some(bucket) => provider.head_bucket(bucket).await.map(|()| TestConnectionResult {
                    method: TestMethod::HeadBucket,
                    bucket_count: None,
                }),
                None => Err(AppError::new(
                    ErrorCode::AccessDenied,
                    "The credentials were accepted but cannot list buckets. Enter a bucket name to test access to a specific bucket.",
                )
                .with_details(err.details.unwrap_or_default())),
            },
            Err(err) => Err(err),
        }
    }

    /// Returns a cached client for a saved connection, building one on miss.
    /// `region` overrides the connection's region (AWS buckets elsewhere).
    fn client_for_saved(
        &self,
        connection: &StorageConnection,
        region: Option<&str>,
    ) -> Result<Client, AppError> {
        let revision = &connection.updated_at;
        if let Some(client) = self.clients.get(&connection.id, region, revision) {
            return Ok(client);
        }
        let secret = self.credentials.get(&connection.credential_ref)?;
        let mut config = resolved_from(connection);
        if let Some(region) = region {
            config.region = region.to_owned();
        }
        let client = build_client(&config, &secret);
        self.clients
            .insert(&connection.id, region, revision, client.clone());
        Ok(client)
    }

    /// Client for connection-wide operations such as ListBuckets.
    pub async fn client(&self, connection_id: &str) -> Result<Client, AppError> {
        self.client_for_saved(&self.load(connection_id).await?, None)
    }

    /// Client able to reach `bucket`. On AWS a bucket may live outside the
    /// connection's region; its region is discovered once and cached, so the
    /// user never has to create one connection per region.
    pub async fn client_for_bucket(
        &self,
        connection_id: &str,
        bucket: &str,
    ) -> Result<Client, AppError> {
        let connection = self.load(connection_id).await?;
        let default_client = self.client_for_saved(&connection, None)?;
        if connection.provider != ProviderType::Aws || connection.endpoint.is_some() {
            return Ok(default_client);
        }

        let region = match self.clients.bucket_region(connection_id, bucket) {
            Some(region) => region,
            None => match S3StorageProvider::new(default_client.clone())
                .bucket_region(bucket)
                .await
            {
                Ok(Some(region)) => {
                    self.clients
                        .set_bucket_region(connection_id, bucket, &region);
                    region
                }
                // Unknown region: use the default client and let the actual
                // request report the precise error.
                Ok(None) | Err(_) => return Ok(default_client),
            },
        };

        if region == connection.region {
            Ok(default_client)
        } else {
            self.client_for_saved(&connection, Some(&region))
        }
    }

    /// Unsaved settings may reuse a saved connection's secret while editing,
    /// as long as the access key ID is unchanged.
    async fn client_for_unsaved(
        &self,
        config: &ConnectionConfigInput,
        saved_id: Option<&str>,
    ) -> Result<Client, AppError> {
        let resolved = resolve_config(config)?;
        let secret = match (
            config.secret_access_key.as_ref().filter(|s| !s.is_blank()),
            saved_id,
        ) {
            (Some(secret), _) => secret.clone(),
            (None, Some(id)) => {
                let saved = self.load(id).await?;
                if saved.access_key_id != resolved.access_key_id {
                    return Err(invalid(
                        "Enter the secret access key for the new access key ID.",
                    ));
                }
                self.credentials.get(&saved.credential_ref)?
            }
            (None, None) => return Err(invalid("Enter the secret access key.")),
        };
        Ok(build_client(&resolved, &secret))
    }

    async fn load(&self, id: &str) -> Result<StorageConnection, AppError> {
        self.repo().get(id).await?.ok_or_else(not_found)
    }

    fn discard_secret(&self, reference: &str) {
        if self.credentials.delete(reference).is_err() {
            // Nothing references this entry any more; it is harmless but
            // cannot be cleaned up automatically.
            tracing::warn!("failed to remove an unused credential store entry");
        }
    }
}

fn resolve_config(input: &ConnectionConfigInput) -> Result<ResolvedConfig, AppError> {
    let region = match input.region.trim() {
        "" => profile(input.provider)
            .default_region
            .ok_or_else(|| invalid("Enter a region."))?
            .to_owned(),
        region => region.to_owned(),
    };
    if region.len() > 64
        || !region
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(invalid(
            "The region may only contain letters, numbers, '-' and '_'.",
        ));
    }

    let access_key_id = input.access_key_id.trim().to_owned();
    if access_key_id.is_empty() {
        return Err(invalid("Enter the access key ID."));
    }
    if access_key_id.len() > MAX_FIELD_LEN || access_key_id.chars().any(char::is_whitespace) {
        return Err(invalid("The access key ID is not valid."));
    }

    let endpoint = resolve_endpoint(
        input.provider,
        input.endpoint.as_deref(),
        &region,
        input.allow_insecure_http,
    )?;

    Ok(ResolvedConfig {
        provider: input.provider,
        endpoint,
        region,
        access_key_id,
        force_path_style: input.force_path_style,
        allow_insecure_http: input.allow_insecure_http,
    })
}

fn resolved_from(c: &StorageConnection) -> ResolvedConfig {
    ResolvedConfig {
        provider: c.provider,
        endpoint: c.endpoint.clone(),
        region: c.region.clone(),
        access_key_id: c.access_key_id.clone(),
        force_path_style: c.force_path_style,
        allow_insecure_http: c.allow_insecure_http,
    }
}

fn validate_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(invalid("Enter a connection name."));
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(invalid(format!(
            "The connection name must be at most {MAX_NAME_LEN} characters."
        )));
    }
    Ok(name.to_owned())
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::ConnectionInvalid, message)
}

fn not_found() -> AppError {
    AppError::new(
        ErrorCode::ConnectionInvalid,
        "The connection no longer exists.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_pool;
    use crate::security::SecretString;
    use crate::security::credential_store::memory::MemoryCredentialStore;

    fn config(secret: Option<&str>) -> ConnectionConfigInput {
        ConnectionConfigInput {
            name: "  Local MinIO ".into(),
            provider: ProviderType::Minio,
            endpoint: Some("http://localhost:9000/".into()),
            region: "".into(),
            access_key_id: "minioadmin".into(),
            secret_access_key: secret.map(SecretString::new),
            force_path_style: true,
            allow_insecure_http: true,
        }
    }

    struct Fixture {
        pool: SqlitePool,
        store: MemoryCredentialStore,
        clients: ClientCache,
    }

    impl Fixture {
        async fn new() -> Self {
            Self {
                pool: test_pool().await,
                store: MemoryCredentialStore::default(),
                clients: ClientCache::default(),
            }
        }

        fn service(&self) -> ConnectionService<'_> {
            ConnectionService::new(&self.pool, &self.store, &self.clients)
        }

        async fn stored_ref(&self, id: &str) -> String {
            sqlx::query_scalar("SELECT credential_ref FROM connections WHERE id = ?")
                .bind(id)
                .fetch_one(&self.pool)
                .await
                .unwrap()
        }
    }

    #[tokio::test]
    async fn create_normalizes_input_and_keeps_secret_out_of_sqlite() {
        let f = Fixture::new().await;
        let dto = f
            .service()
            .create(config(Some("s3cr3t-value")))
            .await
            .unwrap();

        assert_eq!(dto.name, "Local MinIO");
        assert_eq!(dto.region, "us-east-1"); // MinIO profile default
        assert_eq!(dto.endpoint.as_deref(), Some("http://localhost:9000"));

        let reference = f.stored_ref(&dto.id).await;
        assert_eq!(f.store.value(&reference).as_deref(), Some("s3cr3t-value"));

        // DB-008: the secret must not appear anywhere in the database.
        let dump: Vec<String> = sqlx::query_scalar(
            "SELECT name || endpoint || region || access_key_id || credential_ref FROM connections",
        )
        .fetch_all(&f.pool)
        .await
        .unwrap();
        assert!(dump.iter().all(|row| !row.contains("s3cr3t-value")));
    }

    #[tokio::test]
    async fn create_requires_a_secret() {
        let f = Fixture::new().await;
        let err = f.service().create(config(None)).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::ConnectionInvalid);
        assert_eq!(f.store.len(), 0);
    }

    #[tokio::test]
    async fn create_removes_secret_when_insert_fails() {
        let f = Fixture::new().await;
        sqlx::query("DROP TABLE transfers")
            .execute(&f.pool)
            .await
            .unwrap();
        sqlx::query("DROP TABLE connection_buckets")
            .execute(&f.pool)
            .await
            .unwrap();
        sqlx::query("DROP TABLE connections")
            .execute(&f.pool)
            .await
            .unwrap();

        assert!(f.service().create(config(Some("secret"))).await.is_err());
        assert_eq!(f.store.len(), 0, "orphaned secret left in credential store");
    }

    #[tokio::test]
    async fn update_without_secret_keeps_credential() {
        let f = Fixture::new().await;
        let dto = f.service().create(config(Some("secret"))).await.unwrap();
        let reference = f.stored_ref(&dto.id).await;

        let mut changed = config(None);
        changed.name = "Renamed".into();
        let updated = f
            .service()
            .update(UpdateConnectionInput {
                id: dto.id.clone(),
                config: changed,
            })
            .await
            .unwrap();

        assert_eq!(updated.name, "Renamed");
        assert_eq!(f.stored_ref(&dto.id).await, reference);
        assert_eq!(f.store.value(&reference).as_deref(), Some("secret"));
    }

    #[tokio::test]
    async fn update_with_secret_rotates_reference_and_removes_old_secret() {
        let f = Fixture::new().await;
        let dto = f.service().create(config(Some("old"))).await.unwrap();
        let old_ref = f.stored_ref(&dto.id).await;

        f.service()
            .update(UpdateConnectionInput {
                id: dto.id.clone(),
                config: config(Some("new")),
            })
            .await
            .unwrap();

        let new_ref = f.stored_ref(&dto.id).await;
        assert_ne!(new_ref, old_ref);
        assert_eq!(f.store.value(&new_ref).as_deref(), Some("new"));
        assert_eq!(f.store.value(&old_ref), None);
        assert_eq!(f.store.len(), 1);
    }

    #[tokio::test]
    async fn changing_access_key_requires_new_secret() {
        let f = Fixture::new().await;
        let dto = f.service().create(config(Some("secret"))).await.unwrap();

        let mut changed = config(None);
        changed.access_key_id = "another-key".into();
        let err = f
            .service()
            .update(UpdateConnectionInput {
                id: dto.id,
                config: changed,
            })
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::ConnectionInvalid);
    }

    #[tokio::test]
    async fn delete_removes_row_and_secret() {
        let f = Fixture::new().await;
        let dto = f.service().create(config(Some("secret"))).await.unwrap();

        let result = f.service().delete(&dto.id).await.unwrap();

        assert!(result.secret_removed);
        assert_eq!(f.store.len(), 0);
        assert!(f.service().list().await.unwrap().is_empty());
    }

    #[test]
    fn rejects_invalid_fields() {
        let mut c = config(Some("s"));
        c.region = "us east 1".into();
        assert!(resolve_config(&c).is_err());

        let mut c = config(Some("s"));
        c.access_key_id = "   ".into();
        assert!(resolve_config(&c).is_err());

        let mut c = config(Some("s"));
        c.allow_insecure_http = false;
        assert!(resolve_config(&c).is_err(), "http endpoint needs opt-in");

        assert!(validate_name("   ").is_err());
        assert!(validate_name(&"x".repeat(101)).is_err());
    }
}

/// Runs against a real MinIO server when `BUCKLANE_MINIO_ENDPOINT` is set
/// (see docker-compose.yml); otherwise each test returns early as skipped.
#[cfg(test)]
mod minio_tests {
    use super::*;
    use crate::database::test_pool;
    use crate::security::SecretString;
    use crate::security::credential_store::memory::MemoryCredentialStore;

    const ACCESS_KEY: &str = "bucklane-test";
    const SECRET_KEY: &str = "bucklane-test-secret";

    fn minio_endpoint() -> Option<String> {
        let endpoint = std::env::var("BUCKLANE_MINIO_ENDPOINT").ok();
        if endpoint.is_none() {
            eprintln!("skipped: BUCKLANE_MINIO_ENDPOINT not set");
        }
        endpoint
    }

    fn config(endpoint: &str, secret: &str) -> ConnectionConfigInput {
        ConnectionConfigInput {
            name: "MinIO".into(),
            provider: ProviderType::Minio,
            endpoint: Some(endpoint.into()),
            region: "us-east-1".into(),
            access_key_id: ACCESS_KEY.into(),
            secret_access_key: Some(SecretString::new(secret)),
            force_path_style: true,
            allow_insecure_http: true,
        }
    }

    async fn test_with(
        config: ConnectionConfigInput,
        bucket: Option<&str>,
    ) -> Result<TestConnectionResult, AppError> {
        let pool = test_pool().await;
        let store = MemoryCredentialStore::default();
        let clients = ClientCache::default();
        ConnectionService::new(&pool, &store, &clients)
            .test(TestConnectionInput {
                connection_id: None,
                config: Some(config),
                test_bucket: bucket.map(Into::into),
            })
            .await
    }

    #[tokio::test]
    async fn valid_credentials_list_buckets() {
        let Some(endpoint) = minio_endpoint() else {
            return;
        };
        let result = test_with(config(&endpoint, SECRET_KEY), None)
            .await
            .unwrap();
        assert!(matches!(result.method, TestMethod::ListBuckets));
    }

    #[tokio::test]
    async fn wrong_secret_is_an_authentication_error() {
        let Some(endpoint) = minio_endpoint() else {
            return;
        };
        let err = test_with(config(&endpoint, "wrong-secret"), None)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::AuthenticationFailed, "{err:?}");
    }

    #[tokio::test]
    async fn closed_port_is_unreachable() {
        if minio_endpoint().is_none() {
            return;
        }
        let err = test_with(config("http://127.0.0.1:1", SECRET_KEY), None)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::EndpointUnreachable, "{err:?}");
    }

    /// Seeds a bucket with 3 folders and 5 files at the root of `docs/`, then
    /// pages through it two entries at a time.
    #[tokio::test]
    async fn list_objects_paginates_folder_listing() {
        use crate::providers::storage_provider::ListQuery;
        use aws_sdk_s3::primitives::ByteStream;

        let Some(endpoint) = minio_endpoint() else {
            return;
        };
        let resolved = resolve_config(&config(&endpoint, SECRET_KEY)).unwrap();
        let client = build_client(&resolved, &SecretString::new(SECRET_KEY));
        let bucket = format!("bucklane-test-{}", uuid::Uuid::now_v7().simple());
        client.create_bucket().bucket(&bucket).send().await.unwrap();

        let keys = [
            "docs/",
            "docs/a/one.txt",
            "docs/b/two.txt",
            "docs/c/",
            "docs/1.txt",
            "docs/2.txt",
            "docs/3.txt",
            "docs/4.txt",
            "docs/5.txt",
        ];
        for key in keys {
            client
                .put_object()
                .bucket(&bucket)
                .key(key)
                .body(ByteStream::from_static(b"x"))
                .send()
                .await
                .unwrap();
        }

        let provider = S3StorageProvider::new(client);
        let mut token: Option<String> = None;
        let mut names = Vec::new();
        let mut pages = 0;
        loop {
            let page = provider
                .list_objects(ListQuery {
                    bucket: &bucket,
                    prefix: "docs/",
                    delimiter: "/",
                    continuation_token: token.as_deref(),
                    page_size: Some(2),
                })
                .await
                .unwrap();
            pages += 1;
            names.extend(page.objects.into_iter().map(|o| o.name));
            if !page.is_truncated {
                break;
            }
            token = page.next_continuation_token;
        }

        names.sort();
        assert_eq!(
            names,
            [
                "1.txt", "2.txt", "3.txt", "4.txt", "5.txt", "a/", "b/", "c/"
            ]
        );
        assert!(pages >= 4, "expected several pages, got {pages}");
    }

    #[tokio::test]
    async fn head_bucket_reports_missing_bucket() {
        let Some(endpoint) = minio_endpoint() else {
            return;
        };
        let resolved = resolve_config(&config(&endpoint, SECRET_KEY)).unwrap();
        let provider =
            S3StorageProvider::new(build_client(&resolved, &SecretString::new(SECRET_KEY)));
        let err = provider
            .head_bucket("bucklane-does-not-exist")
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::BucketNotFound, "{err:?}");
    }
}
