use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::errors::{AppError, ErrorCode};

pub const PLAN_TTL: Duration = Duration::from_secs(10 * 60);

/// A confirmed-once snapshot of keys under a prefix (TSD §6.3.2).
#[derive(Debug, Clone)]
pub struct StoredPlan {
    pub connection_id: String,
    pub bucket: String,
    pub keys: Vec<String>,
    expires_at: Instant,
}

/// In-memory, single-use folder delete plans. Plans are never persisted, so
/// a restart can never resume a half-confirmed delete.
#[derive(Default)]
pub struct DeletePlanRegistry {
    plans: Mutex<HashMap<String, StoredPlan>>,
}

impl DeletePlanRegistry {
    pub fn insert(&self, connection_id: &str, bucket: &str, keys: Vec<String>) -> String {
        let id = uuid::Uuid::now_v7().to_string();
        let mut plans = self.plans.lock().expect("plan registry poisoned");
        let now = Instant::now();
        plans.retain(|_, p| p.expires_at > now);
        plans.insert(
            id.clone(),
            StoredPlan {
                connection_id: connection_id.to_owned(),
                bucket: bucket.to_owned(),
                keys,
                expires_at: now + PLAN_TTL,
            },
        );
        id
    }

    /// Removes and returns the plan; a plan can be executed only once.
    pub fn take(&self, id: &str) -> Result<StoredPlan, AppError> {
        let plan = self
            .plans
            .lock()
            .expect("plan registry poisoned")
            .remove(id)
            .ok_or_else(expired)?;
        if plan.expires_at <= Instant::now() {
            return Err(expired());
        }
        Ok(plan)
    }
}

fn expired() -> AppError {
    AppError::new(
        ErrorCode::InvalidInput,
        "This delete confirmation has expired. Start the delete again.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_are_single_use() {
        let registry = DeletePlanRegistry::default();
        let id = registry.insert("c1", "b", vec!["a".into()]);

        assert_eq!(registry.take(&id).unwrap().keys, ["a"]);
        assert!(registry.take(&id).is_err());
        assert!(registry.take("unknown").is_err());
    }
}
