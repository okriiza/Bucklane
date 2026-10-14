use serde::Serialize;
use specta::Type;

use crate::domain::connection::ProviderType;

/// Defaults and form hints for a provider (TSD §5.3). Profiles never replace
/// endpoint validation or capability checks.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfile {
    pub provider: ProviderType,
    pub label: &'static str,
    /// The user must supply an endpoint; otherwise it may be derived.
    pub endpoint_required: bool,
    pub endpoint_placeholder: Option<&'static str>,
    pub default_region: Option<&'static str>,
    pub region_placeholder: &'static str,
    pub default_force_path_style: bool,
}

pub fn profile(provider: ProviderType) -> ProviderProfile {
    let base = ProviderProfile {
        provider,
        label: "",
        endpoint_required: true,
        endpoint_placeholder: None,
        default_region: None,
        region_placeholder: "us-east-1",
        default_force_path_style: true,
    };

    match provider {
        ProviderType::Aws => ProviderProfile {
            label: "Amazon S3",
            endpoint_required: false,
            default_force_path_style: false,
            ..base
        },
        ProviderType::Supabase => ProviderProfile {
            label: "Supabase Storage",
            endpoint_placeholder: Some("https://<project-ref>.supabase.co/storage/v1/s3"),
            region_placeholder: "ap-southeast-1",
            ..base
        },
        ProviderType::Minio => ProviderProfile {
            label: "MinIO",
            endpoint_placeholder: Some("https://minio.example.com:9000"),
            default_region: Some("us-east-1"),
            ..base
        },
        ProviderType::CloudflareR2 => ProviderProfile {
            label: "Cloudflare R2",
            endpoint_placeholder: Some("https://<account-id>.r2.cloudflarestorage.com"),
            default_region: Some("auto"),
            region_placeholder: "auto",
            ..base
        },
        ProviderType::Wasabi => ProviderProfile {
            label: "Wasabi",
            endpoint_required: false,
            endpoint_placeholder: Some("https://s3.<region>.wasabisys.com"),
            default_force_path_style: false,
            ..base
        },
        ProviderType::DigitaloceanSpaces => ProviderProfile {
            label: "DigitalOcean Spaces",
            endpoint_required: false,
            endpoint_placeholder: Some("https://<region>.digitaloceanspaces.com"),
            region_placeholder: "sgp1",
            default_force_path_style: false,
            ..base
        },
        ProviderType::Custom => ProviderProfile {
            label: "Custom S3-compatible",
            endpoint_placeholder: Some("https://s3.example.com"),
            ..base
        },
    }
}

pub fn all_profiles() -> Vec<ProviderProfile> {
    ProviderType::ALL.into_iter().map(profile).collect()
}

/// Endpoint used when the user leaves it blank. `None` means the SDK default
/// (AWS regional endpoints).
pub fn derived_endpoint(provider: ProviderType, region: &str) -> Option<String> {
    match provider {
        ProviderType::Wasabi => Some(format!("https://s3.{region}.wasabisys.com")),
        ProviderType::DigitaloceanSpaces => {
            Some(format!("https://{region}.digitaloceanspaces.com"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_provider_has_a_labelled_profile() {
        for p in all_profiles() {
            assert!(!p.label.is_empty(), "{:?} has no label", p.provider);
        }
    }

    #[test]
    fn derives_endpoints_only_for_region_based_providers() {
        assert_eq!(
            derived_endpoint(ProviderType::Wasabi, "eu-central-1").as_deref(),
            Some("https://s3.eu-central-1.wasabisys.com")
        );
        assert_eq!(
            derived_endpoint(ProviderType::DigitaloceanSpaces, "sgp1").as_deref(),
            Some("https://sgp1.digitaloceanspaces.com")
        );
        assert_eq!(derived_endpoint(ProviderType::Minio, "us-east-1"), None);
    }
}
