use serde::Serialize;
use specta::Type;

/// Machine-readable error codes exposed to the frontend (TSD §11).
// Most variants are produced by services added from M01 onward.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    ConnectionInvalid,
    InvalidInput,
    AuthenticationFailed,
    AccessDenied,
    EndpointUnreachable,
    TlsError,
    BucketNotFound,
    ObjectNotFound,
    OperationUnsupported,
    TransferFailed,
    TransferCanceled,
    Interrupted,
    LocalFileError,
    StorageQuotaError,
    DatabaseUnavailable,
    InternalError,
}

impl ErrorCode {
    /// Default retry classification. Individual errors may override it,
    /// e.g. a throttled request reported as `EndpointUnreachable`.
    fn default_retryable(self) -> bool {
        matches!(
            self,
            Self::EndpointUnreachable | Self::TransferFailed | Self::Interrupted
        )
    }
}

/// Structured error returned by every IPC command.
///
/// `message` and `details` must already be sanitized: never include secrets,
/// authorization headers, or signed URLs (SEC-004).
#[derive(Debug, Clone, Serialize, serde::Deserialize, Type, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{code:?}: {message}")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Option<String>,
    pub retryable: bool,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
            retryable: code.default_retryable(),
        }
    }

    #[allow(dead_code)]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InternalError, message)
    }

    /// Rebuilds an error persisted as `error_code` + `error_message`.
    pub fn stored(code: Option<&str>, message: &str) -> Self {
        let code = code
            .and_then(|c| serde_json::from_value(serde_json::Value::String(c.to_owned())).ok())
            .unwrap_or(ErrorCode::InternalError);
        Self::new(code, message)
    }

    /// Code as stored in `transfers.error_code`.
    pub fn code_str(&self) -> String {
        serde_json::to_value(self.code)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "INTERNAL_ERROR".to_owned())
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        // SQLx errors carry no credentials, but keep the frontend message generic
        // and leave the specifics to the local log.
        tracing::error!(error = %err, "database error");
        Self::internal("A local database operation failed.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_screaming_code_and_camel_case_fields() {
        let err = AppError::new(ErrorCode::AccessDenied, "denied").with_details("bucket: photos");
        let json = serde_json::to_value(&err).unwrap();

        assert_eq!(json["code"], "ACCESS_DENIED");
        assert_eq!(json["message"], "denied");
        assert_eq!(json["details"], "bucket: photos");
        assert_eq!(json["retryable"], false);
    }

    #[test]
    fn network_errors_are_retryable_by_default() {
        assert!(AppError::new(ErrorCode::EndpointUnreachable, "x").retryable);
        assert!(!AppError::new(ErrorCode::AuthenticationFailed, "x").retryable);
    }
}
