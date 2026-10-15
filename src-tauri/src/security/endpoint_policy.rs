use url::Url;

use crate::domain::connection::ProviderType;
use crate::errors::{AppError, ErrorCode};
use crate::providers::provider_profiles::{derived_endpoint, profile};

/// Validates and normalizes a connection endpoint (TSD §8.3).
///
/// - Blank endpoints fall back to the provider's derived endpoint, or the SDK
///   default for AWS; providers that need an endpoint reject blanks.
/// - Only `https` is accepted unless `allow_insecure_http` is set.
/// - Credentials, query strings, and fragments in the URL are rejected.
pub fn resolve_endpoint(
    provider: ProviderType,
    raw: Option<&str>,
    region: &str,
    allow_insecure_http: bool,
) -> Result<Option<String>, AppError> {
    let raw = raw.map(str::trim).filter(|s| !s.is_empty());
    let candidate = match raw {
        Some(value) => value.to_owned(),
        None if profile(provider).endpoint_required => {
            return Err(invalid(format!(
                "An endpoint is required for {}.",
                profile(provider).label
            )));
        }
        None => match derived_endpoint(provider, region) {
            Some(derived) => derived,
            None => return Ok(None),
        },
    };

    let url = Url::parse(&candidate)
        .map_err(|_| invalid("The endpoint must be a full URL, e.g. https://s3.example.com."))?;

    match url.scheme() {
        "https" => {}
        "http" if allow_insecure_http => {}
        "http" => {
            return Err(invalid(
                "HTTP endpoints are insecure. Enable \"Allow insecure HTTP\" to use one.",
            ));
        }
        _ => return Err(invalid("The endpoint must use https:// or http://.")),
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(invalid("The endpoint must include a host name."));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(invalid("The endpoint must not contain credentials."));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(invalid(
            "The endpoint must not contain a query string or fragment.",
        ));
    }

    Ok(Some(url.as_str().trim_end_matches('/').to_owned()))
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::ConnectionInvalid, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(
        provider: ProviderType,
        raw: &str,
        allow_http: bool,
    ) -> Result<Option<String>, AppError> {
        resolve_endpoint(provider, Some(raw), "us-east-1", allow_http)
    }

    #[test]
    fn accepts_https_and_trims_trailing_slash() {
        assert_eq!(
            resolve(
                ProviderType::Supabase,
                " https://abc.supabase.co/storage/v1/s3/ ",
                false
            )
            .unwrap()
            .as_deref(),
            Some("https://abc.supabase.co/storage/v1/s3")
        );
    }

    #[test]
    fn rejects_http_without_opt_in() {
        let err = resolve(ProviderType::Minio, "http://localhost:9000", false).unwrap_err();
        assert_eq!(err.code, ErrorCode::ConnectionInvalid);
        assert!(resolve(ProviderType::Minio, "http://localhost:9000", true).is_ok());
    }

    #[test]
    fn rejects_unsafe_or_malformed_urls() {
        for raw in [
            "s3.example.com",
            "ftp://s3.example.com",
            "https://user:pass@s3.example.com",
            "https://s3.example.com?x=1",
            "https://s3.example.com#frag",
        ] {
            assert!(
                resolve(ProviderType::Custom, raw, true).is_err(),
                "{raw} should be rejected"
            );
        }
    }

    #[test]
    fn blank_endpoint_uses_provider_default() {
        assert_eq!(
            resolve_endpoint(ProviderType::Aws, Some("  "), "us-east-1", false).unwrap(),
            None
        );
        assert_eq!(
            resolve_endpoint(ProviderType::Wasabi, None, "us-east-1", false)
                .unwrap()
                .as_deref(),
            Some("https://s3.us-east-1.wasabisys.com")
        );
        assert!(resolve_endpoint(ProviderType::Minio, None, "us-east-1", false).is_err());
    }
}
