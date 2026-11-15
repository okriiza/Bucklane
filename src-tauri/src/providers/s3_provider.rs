use std::error::Error as StdError;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::http::HttpResponse;
use aws_sdk_s3::error::{ProvideErrorMetadata, SdkError};
use aws_sdk_s3::operation::create_multipart_upload::builders::CreateMultipartUploadFluentBuilder;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::primitives::{DateTime, DateTimeFormat};
use aws_sdk_s3::types::{
    CommonPrefix, CompletedMultipartUpload, CompletedPart, Delete, MetadataDirective, Object,
    ObjectCannedAcl, ObjectIdentifier, Permission,
};
use futures_util::{StreamExt, stream};

use crate::domain::bucket::BucketInfo;
use crate::domain::object::{
    DeleteFailure, DeleteObjectsResult, ListObjectsResponse, ObjectHeaders, ObjectMetadata,
    StorageObject,
};
use crate::errors::{AppError, ErrorCode};
use crate::providers::storage_provider::{CopySource, ListQuery, StorageProvider};

const MIB: u64 = 1024 * 1024;
/// S3 limits: parts ≥ 5 MiB (except the last), at most 10,000 parts, and at
/// most 5 GiB per part or single copy.
const MIN_PART_SIZE: u64 = 5 * MIB;
const MAX_PARTS: u64 = 10_000;
const MAX_SINGLE_COPY_BYTES: i64 = 5 * 1024 * 1024 * 1024;
/// Parts of a multipart server-side copy: large, as nothing passes through
/// Bucklane, yet small enough for each request to finish well within the
/// read timeout.
const COPY_PART_SIZE: u64 = 256 * MIB;
/// Parts copied concurrently within one multipart copy.
const COPY_CONCURRENCY: usize = 4;

fn copy_part_size(size: i64) -> u64 {
    part_size_for(u64::try_from(size).unwrap_or(0), COPY_PART_SIZE)
}

/// Part size ≥ the preferred size, large enough to stay within 10,000
/// parts, and rounded up to a whole MiB.
pub(crate) fn part_size_for(size: u64, preferred: u64) -> u64 {
    let needed = size.div_ceil(MAX_PARTS);
    needed.max(preferred).max(MIN_PART_SIZE).div_ceil(MIB) * MIB
}

/// The single S3 adapter used for every provider in the MVP (ADR-005).
pub struct S3StorageProvider {
    client: Client,
}

impl S3StorageProvider {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Copies `source` to `bucket`/`key` in parts of `part_size`, starting
    /// the multipart upload with `create`, which carries the destination's
    /// headers. The upload is aborted if any part fails.
    async fn multipart_copy(
        &self,
        source: CopySource<'_>,
        bucket: &str,
        key: &str,
        create: CreateMultipartUploadFluentBuilder,
        part_size: u64,
    ) -> Result<(), AppError> {
        let created = create
            .send()
            .await
            .map_err(|e| map_sdk_error(e, ErrorCode::BucketNotFound))?;
        let upload_id = created
            .upload_id()
            .ok_or_else(|| AppError::internal("The provider did not return an upload ID."))?
            .to_owned();

        let size = u64::try_from(source.size).unwrap_or(0);
        // After a failure no new part starts, but the ones in flight finish:
        // a part still being copied could otherwise land after the abort
        // and leave the upload behind.
        let failed = AtomicBool::new(false);
        let upload_id_ref = upload_id.as_str();
        let results: Vec<Option<Result<CompletedPart, AppError>>> =
            stream::iter(1..=size.div_ceil(part_size))
                .map(|n| {
                    let failed = &failed;
                    async move {
                        if failed.load(Ordering::SeqCst) {
                            return None;
                        }
                        let offset = (n - 1) * part_size;
                        let len = part_size.min(size - offset);
                        let part_number = i32::try_from(n).expect("at most 10,000 parts");
                        let part = self
                            .upload_part_copy(
                                source,
                                bucket,
                                key,
                                upload_id_ref,
                                part_number,
                                offset,
                                len,
                            )
                            .await;
                        if part.is_err() {
                            failed.store(true, Ordering::SeqCst);
                        }
                        Some(part)
                    }
                })
                .buffer_unordered(COPY_CONCURRENCY)
                .collect()
                .await;
        // Skipped parts (`None`) only follow a failure, which is reported.
        let copied: Result<Vec<CompletedPart>, AppError> = results.into_iter().flatten().collect();
        let completed = match copied {
            Ok(mut parts) => {
                // Completion requires parts in ascending order.
                parts.sort_by_key(|p| p.part_number());
                self.client
                    .complete_multipart_upload()
                    .bucket(bucket)
                    .key(key)
                    .upload_id(&upload_id)
                    .multipart_upload(
                        CompletedMultipartUpload::builder()
                            .set_parts(Some(parts))
                            .build(),
                    )
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|e| map_sdk_error(e, ErrorCode::BucketNotFound))
            }
            Err(err) => Err(err),
        };
        if completed.is_err() {
            let _ = self
                .client
                .abort_multipart_upload()
                .bucket(bucket)
                .key(key)
                .upload_id(&upload_id)
                .send()
                .await;
        }
        completed
    }

    /// Copies `len` bytes of `source` from `offset` into part `part_number`
    /// of a multipart upload, without the bytes passing through Bucklane.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn upload_part_copy(
        &self,
        source: CopySource<'_>,
        bucket: &str,
        key: &str,
        upload_id: &str,
        part_number: i32,
        offset: u64,
        len: u64,
    ) -> Result<CompletedPart, AppError> {
        let output = self
            .client
            .upload_part_copy()
            .bucket(bucket)
            .key(key)
            .upload_id(upload_id)
            .part_number(part_number)
            .copy_source(copy_source(source.bucket, source.key))
            .copy_source_range(format!("bytes={offset}-{}", offset + len - 1))
            .set_copy_source_if_match(source.etag.map(str::to_owned))
            .send()
            .await
            .map_err(|e| map_sdk_error(e, ErrorCode::ObjectNotFound))?;
        let etag = output
            .copy_part_result()
            .and_then(|r| r.e_tag())
            .ok_or_else(|| AppError::internal("The provider did not return a part ETag."))?;
        Ok(CompletedPart::builder()
            .part_number(part_number)
            .e_tag(etag)
            .build())
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
            .set_delimiter(query.delimiter.map(str::to_owned))
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
            cache_control: output.cache_control().map(str::to_owned),
            content_disposition: output.content_disposition().map(str::to_owned),
            content_encoding: output.content_encoding().map(str::to_owned),
            content_language: output.content_language().map(str::to_owned),
            etag: output.e_tag().map(str::to_owned),
            storage_class: output.storage_class().map(|c| c.as_str().to_owned()),
            metadata: output
                .metadata()
                .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
        })
    }

    async fn put_empty_object(&self, bucket: &str, key: &str) -> Result<(), AppError> {
        self.client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(ByteStream::from_static(b""))
            .send()
            .await
            .map(|_| ())
            .map_err(|e| map_sdk_error(e, ErrorCode::BucketNotFound))
    }

    async fn copy_object(
        &self,
        source: CopySource<'_>,
        bucket: &str,
        key: &str,
    ) -> Result<(), AppError> {
        if source.size <= MAX_SINGLE_COPY_BYTES {
            return self
                .client
                .copy_object()
                .bucket(bucket)
                .key(key)
                .copy_source(copy_source(source.bucket, source.key))
                .set_copy_source_if_match(source.etag.map(str::to_owned))
                .send()
                .await
                .map(|_| ())
                .map_err(|e| map_sdk_error(e, ErrorCode::ObjectNotFound));
        }
        // A multipart copy doesn't carry the headers over like a single
        // copy does; set them from the source.
        let current = self
            .client
            .head_object()
            .bucket(source.bucket)
            .key(source.key)
            .set_if_match(source.etag.map(str::to_owned))
            .send()
            .await
            .map_err(|e| map_sdk_error(e, ErrorCode::ObjectNotFound))?;
        let create = self
            .client
            .create_multipart_upload()
            .bucket(bucket)
            .key(key)
            .set_content_type(current.content_type().map(str::to_owned))
            .set_cache_control(current.cache_control().map(str::to_owned))
            .set_content_disposition(current.content_disposition().map(str::to_owned))
            .set_content_encoding(current.content_encoding().map(str::to_owned))
            .set_content_language(current.content_language().map(str::to_owned))
            .set_metadata(current.metadata().cloned());
        self.multipart_copy(source, bucket, key, create, copy_part_size(source.size))
            .await
    }

    async fn get_bytes(
        &self,
        bucket: &str,
        key: &str,
        limit: u64,
    ) -> Result<(Vec<u8>, bool), AppError> {
        // One byte past the limit tells whether the object continues.
        let output = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .range(format!("bytes=0-{limit}"))
            .send()
            .await
            .map_err(|e| map_sdk_error(e, ErrorCode::ObjectNotFound))?;
        let mut body = output.body;
        let mut bytes = Vec::new();
        while let Some(chunk) = body.try_next().await.map_err(|err| {
            AppError::new(
                ErrorCode::TransferFailed,
                "The connection was interrupted while loading the preview.",
            )
            .with_details(err.to_string())
        })? {
            bytes.extend_from_slice(&chunk);
            if bytes.len() as u64 > limit {
                break;
            }
        }
        let more = bytes.len() as u64 > limit;
        bytes.truncate(limit as usize);
        Ok((bytes, more))
    }

    async fn presign_get(
        &self,
        bucket: &str,
        key: &str,
        expires_in: std::time::Duration,
    ) -> Result<String, AppError> {
        let config = PresigningConfig::expires_in(expires_in).map_err(|_| {
            AppError::new(ErrorCode::InvalidInput, "The link duration is not valid.")
        })?;
        let request = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .presigned(config)
            .await
            .map_err(|e| map_sdk_error(e, ErrorCode::ObjectNotFound))?;
        Ok(request.uri().to_owned())
    }

    async fn replace_metadata(
        &self,
        bucket: &str,
        key: &str,
        etag: &str,
        headers: &ObjectHeaders,
    ) -> Result<(), AppError> {
        // A REPLACE copy resets everything it is not given, so carry over the
        // settings the editor does not show.
        let current = self
            .client
            .head_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| map_sdk_error(e, ErrorCode::ObjectNotFound))?;
        let expires = current
            .expires_string()
            .and_then(|s| DateTime::from_str(s, DateTimeFormat::HttpDate).ok());

        // A copy never keeps the object's ACL. Re-apply the common public-read
        // case so a public file stays public; providers without ACLs fail
        // this request and need nothing re-applied.
        let public_read = match self
            .client
            .get_object_acl()
            .bucket(bucket)
            .key(key)
            .send()
            .await
        {
            Ok(acl) => acl.grants().iter().any(|grant| {
                grant.grantee().and_then(|g| g.uri()) == Some(ALL_USERS_URI)
                    && matches!(
                        grant.permission(),
                        Some(Permission::Read | Permission::FullControl)
                    )
            }),
            Err(_) => false,
        };

        let size = current.content_length().unwrap_or(0);
        if size > MAX_SINGLE_COPY_BYTES {
            // Too large for one copy: copy it onto itself in parts, with
            // the same settings a single copy would get.
            let create = self
                .client
                .create_multipart_upload()
                .bucket(bucket)
                .key(key)
                .set_content_type(headers.content_type.clone())
                .set_cache_control(headers.cache_control.clone())
                .set_content_disposition(headers.content_disposition.clone())
                .set_content_encoding(headers.content_encoding.clone())
                .set_content_language(headers.content_language.clone())
                .set_metadata(Some(headers.metadata.clone().into_iter().collect()))
                .set_expires(expires)
                .set_website_redirect_location(
                    current.website_redirect_location().map(str::to_owned),
                )
                .set_storage_class(current.storage_class().cloned())
                .set_server_side_encryption(current.server_side_encryption().cloned())
                .set_ssekms_key_id(current.ssekms_key_id().map(str::to_owned))
                .set_bucket_key_enabled(current.bucket_key_enabled())
                .set_acl(public_read.then_some(ObjectCannedAcl::PublicRead));
            let source = CopySource {
                bucket,
                key,
                etag: Some(etag),
                size,
            };
            return self
                .multipart_copy(source, bucket, key, create, copy_part_size(source.size))
                .await;
        }

        self.client
            .copy_object()
            .bucket(bucket)
            .key(key)
            .copy_source(copy_source(bucket, key))
            .copy_source_if_match(etag)
            .metadata_directive(MetadataDirective::Replace)
            .set_content_type(headers.content_type.clone())
            .set_cache_control(headers.cache_control.clone())
            .set_content_disposition(headers.content_disposition.clone())
            .set_content_encoding(headers.content_encoding.clone())
            .set_content_language(headers.content_language.clone())
            .set_metadata(Some(headers.metadata.clone().into_iter().collect()))
            .set_expires(expires)
            .set_website_redirect_location(current.website_redirect_location().map(str::to_owned))
            .set_storage_class(current.storage_class().cloned())
            .set_server_side_encryption(current.server_side_encryption().cloned())
            .set_ssekms_key_id(current.ssekms_key_id().map(str::to_owned))
            .set_bucket_key_enabled(current.bucket_key_enabled())
            .set_acl(public_read.then_some(ObjectCannedAcl::PublicRead))
            .send()
            .await
            .map(|_| ())
            .map_err(|e| map_sdk_error(e, ErrorCode::ObjectNotFound))
    }

    async fn delete_object(&self, bucket: &str, key: &str) -> Result<(), AppError> {
        self.client
            .delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map(|_| ())
            .map_err(|e| map_sdk_error(e, ErrorCode::ObjectNotFound))
    }

    async fn delete_objects(
        &self,
        bucket: &str,
        keys: &[String],
    ) -> Result<DeleteObjectsResult, AppError> {
        let objects = keys
            .iter()
            .map(|key| ObjectIdentifier::builder().key(key).build())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::internal("Could not build the delete request."))?;
        let delete = Delete::builder()
            .set_objects(Some(objects))
            .quiet(true)
            .build()
            .map_err(|_| AppError::internal("Could not build the delete request."))?;

        let output = self
            .client
            .delete_objects()
            .bucket(bucket)
            .delete(delete)
            .send()
            .await
            .map_err(|e| map_sdk_error(e, ErrorCode::BucketNotFound))?;

        // Quiet mode only reports failures; every other key was deleted.
        let failed: Vec<DeleteFailure> = output
            .errors()
            .iter()
            .map(|e| DeleteFailure {
                key: e.key().unwrap_or_default().to_owned(),
                error: map_service_error(e.code(), 0, ErrorCode::ObjectNotFound)
                    .with_details(e.code().unwrap_or("no error code").to_owned()),
            })
            .collect();
        let deleted = keys
            .iter()
            .filter(|k| !failed.iter().any(|f| &f.key == *k))
            .cloned()
            .collect();
        Ok(DeleteObjectsResult { deleted, failed })
    }
}

const ALL_USERS_URI: &str = "http://acs.amazonaws.com/groups/global/AllUsers";

/// `x-amz-copy-source` value: bucket and URL-encoded key.
fn copy_source(bucket: &str, key: &str) -> String {
    format!("{bucket}/{}", encode_key(key))
}

/// Percent-encodes a key for use in a URL path. `/` is kept so the key's
/// folder structure is preserved.
pub fn encode_key(key: &str) -> String {
    let mut encoded = String::with_capacity(key.len());
    for byte in key.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// Converts one ListObjectsV2 page into explorer entries: folders first, then
/// files. The marker object for the listed folder itself (`prefix` + nothing)
/// is hidden, since it represents the folder the user is already in.
///
/// Names are relative to the folder containing `prefix`, so a name search
/// (`photos/ca`) still yields `cat.jpg` rather than `t.jpg`.
fn listing_entries(
    prefix: &str,
    common_prefixes: &[CommonPrefix],
    contents: &[Object],
) -> Vec<StorageObject> {
    let folder = &prefix[..prefix.rfind('/').map_or(0, |i| i + 1)];
    let relative = |key: &str| key.strip_prefix(folder).unwrap_or(key).to_owned();

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

pub(crate) fn map_service_error(code: Option<&str>, status: u16, not_found: ErrorCode) -> AppError {
    use ErrorCode::*;

    let (error_code, message) = match (code, status) {
        (Some("InvalidAccessKeyId"), _) => (
            AuthenticationFailed,
            "This storage doesn't recognize the access key ID. Check that it was copied completely.",
        ),
        // A mismatched signature has several causes besides a wrong secret;
        // self-hosted setups most often hit region or proxy problems.
        (Some("SignatureDoesNotMatch"), _) => (
            AuthenticationFailed,
            "The request signature was rejected. Check the secret access key, and that the region matches the server's (self-hosted Supabase defaults to \"stub\"). A reverse proxy that changes the Host header or path also causes this.",
        ),
        (Some("RequestTimeTooSkewed"), _) => (
            AuthenticationFailed,
            "Your computer's clock is too far off for the storage to accept requests. Turn on automatic date and time, then try again.",
        ),
        (Some("InvalidSecurity" | "InvalidToken" | "ExpiredToken"), _) | (None, 401) => (
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
        (Some("NoSuchKey" | "NotFound" | "NoSuchUpload"), _) | (None, 404) => (
            not_found,
            if not_found == BucketNotFound {
                "The bucket does not exist or is not accessible."
            } else {
                "The object does not exist."
            },
        ),
        (Some("PreconditionFailed"), _) | (None, 412) => (
            InvalidInput,
            "The object changed on the server since it was read. Reload and try again.",
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
    fn part_size_respects_s3_limits() {
        let preferred = 16 * MIB;
        assert_eq!(part_size_for(100 * MIB, preferred), preferred);
        // Never below the S3 minimum, whatever the preference.
        assert_eq!(part_size_for(100 * MIB, MIB), MIN_PART_SIZE);
        // 1 TiB / 10,000 parts ≈ 105 MiB → rounded up to whole MiB.
        let tib = 1024 * 1024 * MIB;
        let size = part_size_for(tib, preferred);
        assert!(tib.div_ceil(size) <= MAX_PARTS);
        assert_eq!(size % MIB, 0);
        // 5 TiB (S3 maximum object size) still fits.
        assert!((5 * tib).div_ceil(part_size_for(5 * tib, preferred)) <= MAX_PARTS);
    }

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
    fn signature_errors_point_beyond_the_secret() {
        let err = map_service_error(
            Some("SignatureDoesNotMatch"),
            403,
            ErrorCode::BucketNotFound,
        );
        assert_eq!(err.code, ErrorCode::AuthenticationFailed);
        assert!(err.message.contains("region"));
        let skew = map_service_error(Some("RequestTimeTooSkewed"), 403, ErrorCode::BucketNotFound);
        assert!(skew.message.contains("clock"));
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
    fn name_search_keeps_names_relative_to_the_folder() {
        let prefixes = [CommonPrefix::builder().prefix("photos/cats/").build()];
        let contents = [Object::builder().key("photos/cat.jpg").build()];

        let entries = listing_entries("photos/ca", &prefixes, &contents);

        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["cats/", "cat.jpg"]);
        let root = listing_entries("rep", &[], &[Object::builder().key("report.pdf").build()]);
        assert_eq!(root[0].name, "report.pdf");
    }

    #[test]
    fn listing_at_root_keeps_full_keys_as_names() {
        let contents = [Object::builder().key("readme.txt").build()];
        let entries = listing_entries("", &[], &contents);
        assert_eq!(entries[0].name, "readme.txt");
    }

    #[test]
    fn copy_source_encodes_keys_but_keeps_slashes() {
        assert_eq!(
            copy_source("photos", "2026/Summer trip/café #1.jpg"),
            "photos/2026/Summer%20trip/caf%C3%A9%20%231.jpg"
        );
        assert_eq!(copy_source("b", "a+b=c?.txt"), "b/a%2Bb%3Dc%3F.txt");
    }

    #[test]
    fn detects_tls_failures() {
        assert!(looks_like_tls_error(
            "invalid peer certificate: UnknownIssuer"
        ));
        assert!(!looks_like_tls_error("Connection refused (os error 61)"));
    }
}

/// Runs against an S3-compatible server when `BUCKLANE_S3_TEST_ENDPOINT` is
/// set; skipped otherwise.
#[cfg(test)]
mod s3_integration_tests {
    use super::*;
    use crate::domain::connection::{ProviderType, ResolvedConfig};
    use crate::providers::s3_client_factory::build_client;
    use crate::security::SecretString;

    #[tokio::test]
    async fn multipart_copy_assembles_parts_and_cleans_up_on_failure() {
        let Ok(endpoint) = std::env::var("BUCKLANE_S3_TEST_ENDPOINT") else {
            eprintln!("skipped: BUCKLANE_S3_TEST_ENDPOINT not set");
            return;
        };
        let client = build_client(
            &ResolvedConfig {
                provider: ProviderType::Minio,
                endpoint: Some(endpoint),
                region: "us-east-1".into(),
                access_key_id: "bucklane-test".into(),
                force_path_style: true,
                allow_insecure_http: true,
            },
            &SecretString::new("bucklane-test-secret"),
        );
        let provider = S3StorageProvider::new(client.clone());
        let bucket = format!("bucklane-mpcopy-{}", uuid::Uuid::now_v7().simple());
        client.create_bucket().bucket(&bucket).send().await.unwrap();
        let data: Vec<u8> = (0..12 * MIB as u32).map(|i| (i % 251) as u8).collect();
        let put = client
            .put_object()
            .bucket(&bucket)
            .key("source.bin")
            .body(data.clone().into())
            .send()
            .await
            .unwrap();
        let source = CopySource {
            bucket: &bucket,
            key: "source.bin",
            etag: put.e_tag(),
            size: data.len() as i64,
        };

        // 5 MiB parts: three ranged part copies, the last one short.
        let create = client
            .create_multipart_upload()
            .bucket(&bucket)
            .key("copy.bin")
            .content_type("video/mp4");
        provider
            .multipart_copy(source, &bucket, "copy.bin", create, 5 * MIB)
            .await
            .unwrap();
        let copy = client
            .get_object()
            .bucket(&bucket)
            .key("copy.bin")
            .send()
            .await
            .unwrap();
        assert_eq!(copy.content_type(), Some("video/mp4"));
        let bytes = copy.body.collect().await.unwrap().into_bytes();
        assert!(bytes[..] == data[..], "copied bytes differ");

        // A changed source fails the copy and leaves no upload behind.
        let stale = CopySource {
            etag: Some("\"stale\""),
            ..source
        };
        let create = client
            .create_multipart_upload()
            .bucket(&bucket)
            .key("stale.bin");
        provider
            .multipart_copy(stale, &bucket, "stale.bin", create, 5 * MIB)
            .await
            .unwrap_err();
        // Filtered by key: MinIO may list other tests' uploads too.
        let uploads = client
            .list_multipart_uploads()
            .bucket(&bucket)
            .prefix("stale.bin")
            .send()
            .await
            .unwrap();
        assert!(
            uploads
                .uploads()
                .iter()
                .all(|u| u.key() != Some("stale.bin")),
            "the failed upload was aborted"
        );
    }
}
