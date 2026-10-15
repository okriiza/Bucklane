use serde::{Deserialize, Serialize};
use specta::Type;

use crate::security::SecretString;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    Aws,
    Supabase,
    Minio,
    CloudflareR2,
    Wasabi,
    DigitaloceanSpaces,
    Custom,
}

impl ProviderType {
    pub const ALL: [Self; 7] = [
        Self::Aws,
        Self::Supabase,
        Self::Minio,
        Self::CloudflareR2,
        Self::Wasabi,
        Self::DigitaloceanSpaces,
        Self::Custom,
    ];

    /// Value stored in `connections.provider`; must match the CHECK constraint.
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Aws => "aws",
            Self::Supabase => "supabase",
            Self::Minio => "minio",
            Self::CloudflareR2 => "cloudflare_r2",
            Self::Wasabi => "wasabi",
            Self::DigitaloceanSpaces => "digitalocean_spaces",
            Self::Custom => "custom",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_db_str() == value)
    }
}

/// A persisted connection. Holds a reference to the secret, never the secret.
#[derive(Debug, Clone)]
pub struct StorageConnection {
    pub id: String,
    pub name: String,
    pub provider: ProviderType,
    pub endpoint: Option<String>,
    pub region: String,
    pub access_key_id: String,
    pub credential_ref: String,
    pub force_path_style: bool,
    pub allow_insecure_http: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Connection data sent to the frontend; excludes the secret and its reference.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionDto {
    pub id: String,
    pub name: String,
    pub provider: ProviderType,
    pub endpoint: Option<String>,
    pub region: String,
    pub access_key_id: String,
    pub force_path_style: bool,
    pub allow_insecure_http: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<StorageConnection> for ConnectionDto {
    fn from(c: StorageConnection) -> Self {
        Self {
            id: c.id,
            name: c.name,
            provider: c.provider,
            endpoint: c.endpoint,
            region: c.region,
            access_key_id: c.access_key_id,
            force_path_style: c.force_path_style,
            allow_insecure_http: c.allow_insecure_http,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

/// Connection settings shared by create, update, and test requests.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionConfigInput {
    pub name: String,
    pub provider: ProviderType,
    pub endpoint: Option<String>,
    pub region: String,
    pub access_key_id: String,
    /// Required on create. On update, absent or blank keeps the stored secret.
    pub secret_access_key: Option<SecretString>,
    pub force_path_style: bool,
    pub allow_insecure_http: bool,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConnectionInput {
    pub id: String,
    #[serde(flatten)]
    pub config: ConnectionConfigInput,
}

/// Tests either a saved connection, an unsaved configuration, or (while
/// editing) an unsaved configuration that reuses the saved connection's secret.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionInput {
    pub connection_id: Option<String>,
    pub config: Option<ConnectionConfigInput>,
    pub test_bucket: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TestMethod {
    ListBuckets,
    HeadBucket,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionResult {
    pub method: TestMethod,
    /// Number of visible buckets when `method` is `LIST_BUCKETS`.
    pub bucket_count: Option<u32>,
}

/// Validated, normalized connection settings ready to build a client.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub provider: ProviderType,
    pub endpoint: Option<String>,
    pub region: String,
    pub access_key_id: String,
    pub force_path_style: bool,
    pub allow_insecure_http: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_db_strings_round_trip() {
        for provider in ProviderType::ALL {
            assert_eq!(
                ProviderType::from_db_str(provider.as_db_str()),
                Some(provider)
            );
        }
    }

    #[test]
    fn provider_serde_matches_db_strings() {
        for provider in ProviderType::ALL {
            let json = serde_json::to_value(provider).unwrap();
            assert_eq!(json, provider.as_db_str());
        }
    }
}
