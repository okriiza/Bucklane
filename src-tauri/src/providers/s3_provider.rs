use std::error::Error as StdError;

use async_trait::async_trait;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::http::HttpResponse;
use aws_sdk_s3::error::{ProvideErrorMetadata, SdkError};

use crate::domain::bucket::BucketInfo;
use crate::errors::{AppError, ErrorCode};
use crate::providers::storage_provider::StorageProvider;

/// The single S3 adapter used for every provider in the MVP (ADR-005).
pub struct S3StorageProvider {
    client: Client,
}

impl S3StorageProvider {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StorageProvider for S3StorageProvider {
    async fn list_buckets(&self) -> Result<Vec<BucketInfo>, AppError> {
        let mut buckets = Vec::new();
        let mut pages = self.client.list_buckets().into_paginator().send();

        while let Some(page) = pages.next().await {
            let page = page.map_err(|e| map_sdk_error(e, ErrorCode::BucketNotFound))?;
            buckets.extend(page.buckets().iter().map(|b| BucketInfo {
                name: b.name().unwrap_or_default().to_owned(),
                created_at: b.creation_date().map(|d| d.to_string()),
            }));
        }

        Ok(buckets)
    }

    async fn head_bucket(&self, bucket: &str) -> Result<(), AppError> {
        self.client
            .head_bucket()
            .bucket(bucket)
            .send()
            .await
            .map(|_| ())
            .map_err(|e| map_sdk_error(e, ErrorCode::BucketNotFound))
    }
}

/// Translates SDK failures into actionable, sanitized errors (TSD §11).
///
/// `not_found` is the code used for a bare HTTP 404 without an S3 error code
/// (e.g. HeadBucket/HeadObject responses have no body).
pub fn map_sdk_error<E>(err: SdkError<E, HttpResponse>, not_found: ErrorCode) -> AppError
where
    E: ProvideErrorMetadata + StdError + Send + Sync + 'static,
{
    match &err {
        SdkError::TimeoutError(_) => AppError::new(
            ErrorCode::EndpointUnreachable,
            "The storage endpoint did not respond in time.",
        ),
        SdkError::DispatchFailure(failure) => {
            let cause = root_cause(&err);
            if looks_like_tls_error(&cause) {
                AppError::new(
                    ErrorCode::TlsError,
                    "A secure connection could not be established. Check the endpoint's TLS certificate.",
                )
                .with_details(cause)
            } else if failure.is_timeout() {
                AppError::new(
                    ErrorCode::EndpointUnreachable,
                    "The storage endpoint did not respond in time.",
                )
            } else {
                AppError::new(
                    ErrorCode::EndpointUnreachable,
                    "Could not reach the storage endpoint. Check the endpoint URL and your network connection.",
                )
                .with_details(cause)
            }
        }
        SdkError::ResponseError(_) => AppError::new(
            ErrorCode::ConnectionInvalid,
            "The endpoint did not respond like an S3 API. Check the endpoint URL.",
        ),
        SdkError::ServiceError(service) => {
            let status = service.raw().status().as_u16();
            let code = service.err().code();
            map_service_error(code, status, not_found).with_details(format!(
                "{} (HTTP {status})",
                code.unwrap_or("no error code")
            ))
        }
        _ => AppError::internal("The storage request could not be built."),
    }
}

fn map_service_error(code: Option<&str>, status: u16, not_found: ErrorCode) -> AppError {
    use ErrorCode::*;

    let (error_code, message) = match (code, status) {
        (Some("InvalidAccessKeyId" | "SignatureDoesNotMatch" | "InvalidSecurity"), _)
        | (None, 401) => (
            AuthenticationFailed,
            "The access key ID or secret access key is incorrect.",
        ),
        (Some("AuthorizationHeaderMalformed" | "PermanentRedirect" | "IncorrectEndpoint"), _)
        | (None, 301) => (
            ConnectionInvalid,
            "The region or endpoint does not match this storage account.",
        ),
        (Some("AccessDenied" | "AllAccessDisabled" | "AccountProblem"), _) | (None, 403) => (
            AccessDenied,
            "Access denied. The credentials are valid but lack permission for this operation.",
        ),
        (Some("NoSuchBucket"), _) => (BucketNotFound, "The bucket does not exist."),
        (Some("NoSuchKey" | "NotFound"), _) | (None, 404) => (
            not_found,
            if not_found == BucketNotFound {
                "The bucket does not exist or is not accessible."
            } else {
                "The object does not exist."
            },
        ),
        (Some("NotImplemented" | "MethodNotAllowed"), _) | (None, 405 | 501) => (
            OperationUnsupported,
            "This storage provider does not support the requested operation.",
        ),
        (Some("QuotaExceeded" | "StorageQuotaExceeded"), _) => {
            (StorageQuotaError, "The storage quota has been exceeded.")
        }
        (Some("SlowDown" | "ServiceUnavailable" | "InternalError"), _) | (_, 500..=599) => {
            let err = AppError::new(
                EndpointUnreachable,
                "The storage provider is temporarily unavailable. Try again shortly.",
            );
            return AppError {
                retryable: true,
                ..err
            };
        }
        _ => (
            InternalError,
            "The storage provider returned an unexpected error.",
        ),
    };

    AppError::new(error_code, message)
}

fn root_cause(err: &(dyn StdError + 'static)) -> String {
    let mut current = err;
    while let Some(source) = current.source() {
        current = source;
    }
    current.to_string()
}

fn looks_like_tls_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    ["certificate", "tls", "ssl", "handshake"]
        .iter()
        .any(|needle| lower.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_service_errors() {
        let cases = [
            (
                Some("InvalidAccessKeyId"),
                403,
                ErrorCode::AuthenticationFailed,
            ),
            (
                Some("SignatureDoesNotMatch"),
                403,
                ErrorCode::AuthenticationFailed,
            ),
            (Some("AccessDenied"), 403, ErrorCode::AccessDenied),
            (Some("NoSuchBucket"), 404, ErrorCode::BucketNotFound),
            (None, 404, ErrorCode::BucketNotFound),
            (None, 301, ErrorCode::ConnectionInvalid),
            (Some("NotImplemented"), 501, ErrorCode::OperationUnsupported),
            (Some("Weird"), 418, ErrorCode::InternalError),
        ];
        for (code, status, expected) in cases {
            assert_eq!(
                map_service_error(code, status, ErrorCode::BucketNotFound).code,
                expected,
                "{code:?} {status}"
            );
        }
    }

    #[test]
    fn throttling_and_server_errors_are_retryable() {
        let err = map_service_error(Some("SlowDown"), 503, ErrorCode::BucketNotFound);
        assert_eq!(err.code, ErrorCode::EndpointUnreachable);
        assert!(err.retryable);
        assert!(map_service_error(None, 502, ErrorCode::BucketNotFound).retryable);
    }

    #[test]
    fn detects_tls_failures() {
        assert!(looks_like_tls_error(
            "invalid peer certificate: UnknownIssuer"
        ));
        assert!(!looks_like_tls_error("Connection refused (os error 61)"));
    }
}
