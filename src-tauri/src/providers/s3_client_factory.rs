use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use aws_sdk_s3::Client;
use aws_sdk_s3::config::retry::RetryConfig;
use aws_sdk_s3::config::timeout::TimeoutConfig;
use aws_sdk_s3::config::{
    BehaviorVersion, Credentials, Region, RequestChecksumCalculation, ResponseChecksumValidation,
};

use crate::domain::connection::{ProviderType, ResolvedConfig};
use crate::security::SecretString;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_ATTEMPTS: u32 = 3;

/// Builds an S3 client from explicit settings only. The default AWS
/// credential chain (env vars, profiles, IMDS) is never consulted (TSD §5.2).
pub fn build_client(config: &ResolvedConfig, secret: &SecretString) -> Client {
    let credentials = Credentials::new(
        &config.access_key_id,
        secret.expose(),
        None,
        None,
        "bucklane-connection",
    );

    let mut builder = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new(config.region.clone()))
        .credentials_provider(credentials)
        .force_path_style(config.force_path_style)
        .timeout_config(
            TimeoutConfig::builder()
                .connect_timeout(CONNECT_TIMEOUT)
                .read_timeout(READ_TIMEOUT)
                .build(),
        )
        .retry_config(RetryConfig::standard().with_max_attempts(MAX_ATTEMPTS));

    if let Some(endpoint) = &config.endpoint {
        builder = builder.endpoint_url(endpoint);
    }

    // Default CRC checksums on every request are an AWS-specific behavior
    // that many S3-compatible services reject; only send them when required.
    if config.provider != ProviderType::Aws {
        builder = builder
            .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
            .response_checksum_validation(ResponseChecksumValidation::WhenRequired);
    }

    Client::from_conf(builder.build())
}

/// In-memory clients keyed by connection ID and its `updated_at` revision, so
/// an edited connection never reuses a client built from stale settings.
#[derive(Default)]
pub struct ClientCache {
    clients: Mutex<HashMap<String, (String, Client)>>,
}

impl ClientCache {
    pub fn get(&self, connection_id: &str, revision: &str) -> Option<Client> {
        let clients = self.clients.lock().expect("client cache poisoned");
        clients
            .get(connection_id)
            .filter(|(cached_revision, _)| cached_revision == revision)
            .map(|(_, client)| client.clone())
    }

    pub fn insert(&self, connection_id: &str, revision: &str, client: Client) {
        self.clients
            .lock()
            .expect("client cache poisoned")
            .insert(connection_id.to_owned(), (revision.to_owned(), client));
    }

    pub fn invalidate(&self, connection_id: &str) {
        self.clients
            .lock()
            .expect("client cache poisoned")
            .remove(connection_id);
    }
}
