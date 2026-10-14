use std::error::Error as StdError;

use async_trait::async_trait;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::http::HttpResponse;
use aws_sdk_s3::error::{ProvideErrorMetadata, SdkError};
use aws_sdk_s3::primitives::{DateTime, DateTimeFormat};
use aws_sdk_s3::types::{CommonPrefix, Object};

use crate::domain::bucket::BucketInfo;
use crate::domain::object::{ListObjectsResponse, ObjectMetadata, StorageObject};
use crate::errors::{AppError, ErrorCode};
use crate::providers::storage_provider::{ListQuery, StorageProvider};

/// The single S3 adapter used for every provider in the MVP (ADR-005).
pub struct S3StorageProvider {
    client: Client,
}

impl S3StorageProvider {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl S3StorageProvider {
    /// Region a bucket lives in, read from `x-amz-bucket-region`. AWS returns
    /// the header even when the request was sent to the wrong region, which
    /// lets the caller retry with a correctly configured client.
    pub async fn bucket_region(&self, bucket: &str) -> Result<Option<String>, AppError> {
        match self.client.head_bucket().bucket(bucket).send().await {
            Ok(output) => Ok(output.bucket_region().map(str::to_owned)),
            Err(err) => {
                let header = err
                    .raw_response()
                    .and_then(|r| r.headers().get(BUCKET_REGION_HEADER))
                    .map(str::to_owned);
                match header {
                    Some(region) => Ok(Some(region)),
                    None => Err(map_sdk_error(err, ErrorCode::BucketNotFound)),
                }
            }
        }
    }
}

const BUCKET_REGION_HEADER: &str = "x-amz-bucket-region";
const DEFAULT_PAGE_SIZE: i32 = 100;
const MAX_PAGE_SIZE: i32 = 1000;

#[async_trait]
impl StorageProvider for S3StorageProvider {
    async fn list_buckets(&self) -> Result<Vec<BucketInfo>, AppError> {
        let mut buckets = Vec::new();
        let mut pages = self.client.list_buckets().into_paginator().send();

        while let Some(page) = pages.next().await {
            let page = page.map_err(|e| map_sdk_error(e, ErrorCode::BucketNotFound))?;
            buckets.extend(page.buckets().iter().map(|b| BucketInfo {
                name: b.name().unwrap_or_default().to_owned(),
                created_at: b.creation_date().and_then(format_date),
                region: b.bucket_region().map(str::to_owned),
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

    async fn list_objects(&self, query: ListQuery<'_>) -> Result<ListObjectsResponse, AppError> {
        let page_size = query
            .page_size
            .map_or(DEFAULT_PAGE_SIZE, |n| {
                i32::try_from(n).unwrap_or(MAX_PAGE_SIZE)
            })
            .clamp(1, MAX_PAGE_SIZE);

        let output = self
            .client
            .list_objects_v2()
            .bucket(query.bucket)
            .prefix(query.prefix)
            .delimiter(query.delimiter)
            .set_continuation_token(query.continuation_token.map(str::to_owned))
            .max_keys(page_size)
            .send()
            .await
            .map_err(|e| map_sdk_error(e, ErrorCode::BucketNotFound))?;

        Ok(ListObjectsResponse {
            objects: listing_entries(query.prefix, output.common_prefixes(), output.contents()),
            next_continuation_token: output.next_continuation_token().map(str::to_owned),
            is_truncated: output.is_truncated().unwrap_or(false),
        })
    }

    async fn head_object(&self, bucket: &str, key: &str) -> Result<ObjectMetadata, AppError> {
        let output = self
            .client
            .head_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| map_sdk_error(e, ErrorCode::ObjectNotFound))?;

        Ok(ObjectMetadata {
            key: key.to_owned(),
            size: output.content_length(),
            last_modified: output.last_modified().and_then(format_date),
            content_type: output.content_type().map(str::to_owned),
            etag: output.e_tag().map(str::to_owned),
            storage_class: output.storage_class().map(|c| c.as_str().to_owned()),
            metadata: output
                .metadata()
                .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
        })
    }
}

/// Converts one ListObjectsV2 page into explorer entries: folders first, then
/// files. The marker object for the listed folder itself (`prefix` + nothing)
/// is hidden, since it represents the folder the user is already in.
fn listing_entries(
    prefix: &str,
    common_prefixes: &[CommonPrefix],
    contents: &[Object],
) -> Vec<StorageObject> {
    let relative = |key: &str| key.strip_prefix(prefix).unwrap_or(key).to_owned();

    let folders = common_prefixes
        .iter()
        .filter_map(|p| p.prefix())
        .map(|key| StorageObject {
            key: key.to_owned(),
            name: relative(key),
            size: None,
            last_modified: None,
            etag: None,
            storage_class: None,
            is_prefix: true,
        });

    let files = contents
        .iter()
        .filter_map(|o| o.key().map(|key| (key, o)))
        .filter(|(key, _)| *key != prefix)
        .map(|(key, o)| StorageObject {
            key: key.to_owned(),
            name: relative(key),
            size: o.size(),
            last_modified: o.last_modified().and_then(format_date),
            etag: o.e_tag().map(str::to_owned),
            storage_class: o.storage_class().map(|c| c.as_str().to_owned()),
            is_prefix: false,
        });

    folders.chain(files).collect()
}

fn format_date(date: &DateTime) -> Option<String> {
    date.fmt(DateTimeFormat::DateTime).ok()
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
    fn listing_puts_folders_first_and_hides_current_folder_marker() {
        let prefixes = [CommonPrefix::builder().prefix("photos/2026/").build()];
        let contents = [
            Object::builder().key("photos/").size(0).build(),
            Object::builder().key("photos/cat.jpg").size(2048).build(),
        ];

        let entries = listing_entries("photos/", &prefixes, &contents);

        let summary: Vec<_> = entries
            .iter()
            .map(|e| (e.name.as_str(), e.is_prefix, e.size))
            .collect();
        assert_eq!(
            summary,
            [("2026/", true, None), ("cat.jpg", false, Some(2048))]
        );
        assert_eq!(entries[1].key, "photos/cat.jpg");
    }

    #[test]
    fn listing_at_root_keeps_full_keys_as_names() {
        let contents = [Object::builder().key("readme.txt").build()];
        let entries = listing_entries("", &[], &contents);
        assert_eq!(entries[0].name, "readme.txt");
    }

    #[test]
    fn detects_tls_failures() {
        assert!(looks_like_tls_error(
            "invalid peer certificate: UnknownIssuer"
        ));
        assert!(!looks_like_tls_error("Connection refused (os error 61)"));
    }
}
