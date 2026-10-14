use crate::errors::{AppError, ErrorCode};
use crate::security::SecretString;

/// Keychain service name under which every Bucklane secret is stored.
const SERVICE: &str = "app.bucklane.desktop";

/// OS-backed secret storage (TSD §8.1). SQLite only ever holds the reference.
pub trait CredentialStore: Send + Sync {
    fn save(&self, reference: &str, secret: &SecretString) -> Result<(), AppError>;
    fn get(&self, reference: &str) -> Result<SecretString, AppError>;
    /// Deleting a missing entry is not an error.
    fn delete(&self, reference: &str) -> Result<(), AppError>;
}

/// Creates a fresh, opaque reference. A new one is used on every rotation so
/// the old secret stays readable until the database points elsewhere.
pub fn new_reference() -> String {
    format!("connection:{}", uuid::Uuid::now_v7())
}

/// macOS Keychain, Windows Credential Manager, or Linux Secret Service.
/// Never falls back to plaintext when the platform store is unavailable.
pub struct KeyringCredentialStore;

impl KeyringCredentialStore {
    fn entry(reference: &str) -> Result<keyring::Entry, AppError> {
        keyring::Entry::new(SERVICE, reference).map_err(map_keyring_error)
    }
}

impl CredentialStore for KeyringCredentialStore {
    fn save(&self, reference: &str, secret: &SecretString) -> Result<(), AppError> {
        Self::entry(reference)?
            .set_password(secret.expose())
            .map_err(map_keyring_error)
    }

    fn get(&self, reference: &str) -> Result<SecretString, AppError> {
        Self::entry(reference)?
            .get_password()
            .map(SecretString::new)
            .map_err(map_keyring_error)
    }

    fn delete(&self, reference: &str) -> Result<(), AppError> {
        match Self::entry(reference)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(map_keyring_error(err)),
        }
    }
}

fn map_keyring_error(err: keyring::Error) -> AppError {
    // keyring errors never contain the secret value, only platform details.
    tracing::error!(error = %err, "credential store error");
    match err {
        keyring::Error::NoEntry => AppError::new(
            ErrorCode::ConnectionInvalid,
            "The saved secret for this connection is missing. Edit the connection and enter the secret access key again.",
        ),
        keyring::Error::NoDefaultStore | keyring::Error::NoStorageAccess(_) => AppError::new(
            ErrorCode::InternalError,
            "The system credential store is unavailable. On Linux, install and unlock GNOME Keyring or KWallet.",
        ),
        _ => AppError::internal("The system credential store reported an error."),
    }
}

#[cfg(test)]
pub mod memory {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;

    /// In-memory store for tests; optionally fails on save to exercise rollback.
    #[derive(Default)]
    pub struct MemoryCredentialStore {
        pub secrets: Mutex<HashMap<String, String>>,
    }

    impl MemoryCredentialStore {
        pub fn len(&self) -> usize {
            self.secrets.lock().unwrap().len()
        }

        pub fn value(&self, reference: &str) -> Option<String> {
            self.secrets.lock().unwrap().get(reference).cloned()
        }
    }

    impl CredentialStore for MemoryCredentialStore {
        fn save(&self, reference: &str, secret: &SecretString) -> Result<(), AppError> {
            self.secrets
                .lock()
                .unwrap()
                .insert(reference.to_owned(), secret.expose().to_owned());
            Ok(())
        }

        fn get(&self, reference: &str) -> Result<SecretString, AppError> {
            self.value(reference)
                .map(SecretString::new)
                .ok_or_else(|| AppError::new(ErrorCode::ConnectionInvalid, "missing secret"))
        }

        fn delete(&self, reference: &str) -> Result<(), AppError> {
            self.secrets.lock().unwrap().remove(reference);
            Ok(())
        }
    }
}
