use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::domain::settings::AppSettings;
use crate::errors::AppError;
use crate::repositories::settings_repository::SettingsRepository;

// Storage keys from DB design §5; values are JSON-encoded.
const THEME: &str = "appearance.theme";
const MAX_CONCURRENT: &str = "transfers.max_concurrent";
const MULTIPART_THRESHOLD: &str = "transfers.multipart_threshold_mb";
const PART_SIZE: &str = "transfers.multipart_part_size_mb";
const PAGE_SIZE: &str = "explorer.page_size";
const CHECK_UPDATES: &str = "updates.check_on_startup";
const AUTO_RESUME: &str = "transfers.resume_automatically";

pub struct SettingsService<'a> {
    repo: SettingsRepository<'a>,
}

impl<'a> SettingsService<'a> {
    pub fn new(repo: SettingsRepository<'a>) -> Self {
        Self { repo }
    }

    /// Stored values override defaults; a missing or invalid stored value
    /// falls back to its default instead of failing the whole load.
    pub async fn get(&self) -> Result<AppSettings, AppError> {
        let stored = self.repo.all().await?;
        let mut settings = AppSettings::default();
        read(&stored, THEME, &mut settings.theme);
        read(
            &stored,
            MAX_CONCURRENT,
            &mut settings.max_concurrent_transfers,
        );
        read(
            &stored,
            MULTIPART_THRESHOLD,
            &mut settings.multipart_threshold_mb,
        );
        read(&stored, PART_SIZE, &mut settings.multipart_part_size_mb);
        read(&stored, PAGE_SIZE, &mut settings.explorer_page_size);
        read(&stored, CHECK_UPDATES, &mut settings.check_for_updates);
        read(
            &stored,
            AUTO_RESUME,
            &mut settings.resume_downloads_automatically,
        );
        if settings.validate().is_err() {
            tracing::warn!("stored settings out of range; using defaults");
            return Ok(AppSettings::default());
        }
        Ok(settings)
    }

    pub async fn update(&self, settings: AppSettings) -> Result<AppSettings, AppError> {
        settings.validate()?;
        self.repo
            .put_all(&[
                (THEME, json(&settings.theme)),
                (MAX_CONCURRENT, json(&settings.max_concurrent_transfers)),
                (MULTIPART_THRESHOLD, json(&settings.multipart_threshold_mb)),
                (PART_SIZE, json(&settings.multipart_part_size_mb)),
                (PAGE_SIZE, json(&settings.explorer_page_size)),
                (CHECK_UPDATES, json(&settings.check_for_updates)),
                (AUTO_RESUME, json(&settings.resume_downloads_automatically)),
            ])
            .await?;
        Ok(settings)
    }

    pub async fn reset(&self) -> Result<AppSettings, AppError> {
        self.repo.delete_all().await?;
        Ok(AppSettings::default())
    }
}

fn read<T: DeserializeOwned>(
    stored: &std::collections::HashMap<String, String>,
    key: &str,
    target: &mut T,
) {
    if let Some(value) = stored.get(key) {
        match serde_json::from_str(value) {
            Ok(parsed) => *target = parsed,
            Err(_) => tracing::warn!(key, "ignoring invalid stored setting"),
        }
    }
}

fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("settings values serialize")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_pool;
    use crate::domain::settings::Theme;

    #[tokio::test]
    async fn round_trips_and_resets() {
        let pool = test_pool().await;
        let service = SettingsService::new(SettingsRepository::new(&pool));
        assert_eq!(service.get().await.unwrap(), AppSettings::default());

        let custom = AppSettings {
            theme: Theme::Dark,
            max_concurrent_transfers: 5,
            ..Default::default()
        };
        service.update(custom.clone()).await.unwrap();
        assert_eq!(service.get().await.unwrap(), custom);

        // Stored as JSON under the documented keys.
        let raw = SettingsRepository::new(&pool).all().await.unwrap();
        assert_eq!(raw.get(THEME).map(String::as_str), Some("\"dark\""));

        assert_eq!(service.reset().await.unwrap(), AppSettings::default());
        assert_eq!(service.get().await.unwrap(), AppSettings::default());
    }

    #[tokio::test]
    async fn rejects_invalid_updates_and_survives_bad_stored_values() {
        let pool = test_pool().await;
        let service = SettingsService::new(SettingsRepository::new(&pool));
        let invalid = AppSettings {
            max_concurrent_transfers: 99,
            ..Default::default()
        };
        assert!(service.update(invalid).await.is_err());

        SettingsRepository::new(&pool)
            .put_all(&[(THEME, "\"neon\"".into())])
            .await
            .unwrap();
        assert_eq!(service.get().await.unwrap().theme, Theme::System);
    }
}
