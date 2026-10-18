use serde::{Deserialize, Serialize};
use specta::Type;

use crate::errors::{AppError, ErrorCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

/// User preferences (DB design §5). Every field here is applied by the app;
/// options that would weaken safety (e.g. skipping delete confirmations) are
/// intentionally not configurable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: Theme,
    pub max_concurrent_transfers: u32,
    pub multipart_threshold_mb: u32,
    pub multipart_part_size_mb: u32,
    pub explorer_page_size: u32,
    /// Look for a new release on startup. Manual checks always work.
    pub check_for_updates: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            max_concurrent_transfers: 3,
            multipart_threshold_mb: 64,
            multipart_part_size_mb: 16,
            explorer_page_size: 100,
            check_for_updates: true,
        }
    }
}

pub const CONCURRENCY_RANGE: (u32, u32) = (1, 10);
/// S3 parts must be ≥ 5 MiB; 5 GiB is the S3 maximum part size.
pub const PART_SIZE_RANGE_MB: (u32, u32) = (5, 5120);
pub const THRESHOLD_RANGE_MB: (u32, u32) = (5, 5120);
/// ListObjectsV2 returns at most 1000 keys per request.
pub const PAGE_SIZE_RANGE: (u32, u32) = (50, 1000);

impl AppSettings {
    pub fn validate(&self) -> Result<(), AppError> {
        check(
            "Concurrent transfers",
            self.max_concurrent_transfers,
            CONCURRENCY_RANGE,
        )?;
        check(
            "Multipart threshold (MB)",
            self.multipart_threshold_mb,
            THRESHOLD_RANGE_MB,
        )?;
        check(
            "Multipart part size (MB)",
            self.multipart_part_size_mb,
            PART_SIZE_RANGE_MB,
        )?;
        check("Items per page", self.explorer_page_size, PAGE_SIZE_RANGE)
    }
}

fn check(label: &str, value: u32, (min, max): (u32, u32)) -> Result<(), AppError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(AppError::new(
            ErrorCode::InvalidInput,
            format!("{label} must be between {min} and {max}."),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        AppSettings::default().validate().unwrap();
    }

    #[test]
    fn rejects_out_of_range_values() {
        let s = AppSettings {
            max_concurrent_transfers: 0,
            ..Default::default()
        };
        assert!(s.validate().is_err());
        let s = AppSettings {
            multipart_part_size_mb: 4,
            ..Default::default()
        };
        assert!(s.validate().is_err());
        let s = AppSettings {
            explorer_page_size: 1001,
            ..Default::default()
        };
        assert!(s.validate().is_err());
    }
}
