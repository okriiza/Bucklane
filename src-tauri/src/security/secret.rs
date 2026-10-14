use std::fmt;

use serde::Deserialize;
use specta::Type;

/// A secret received over IPC (e.g. a secret access key).
///
/// It can be deserialized but never serialized, and its `Debug` output is
/// redacted so it cannot leak through logs or error formatting (SEC-004).
#[derive(Clone, Deserialize, Type)]
#[serde(transparent)]
#[specta(transparent)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_blank(&self) -> bool {
        self.0.trim().is_empty()
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_is_redacted() {
        let secret = SecretString::new("wJalrXUtnFEMI/K7MDENG");
        assert_eq!(format!("{secret:?}"), "SecretString([REDACTED])");
    }
}
