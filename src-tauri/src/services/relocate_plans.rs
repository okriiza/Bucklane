use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::domain::object::RelocateMode;
use crate::errors::{AppError, ErrorCode};
use crate::services::delete_plans::PLAN_TTL;

/// One object of a folder copy or move.
#[derive(Debug, Clone)]
pub struct RelocateItem {
    pub source: String,
    pub destination: String,
    pub size: i64,
    pub etag: Option<String>,
    /// An object already exists at `destination`.
    pub conflict: bool,
}

#[derive(Debug, Clone)]
pub struct RelocatePlan {
    pub connection_id: String,
    pub bucket: String,
    pub mode: RelocateMode,
    pub items: Vec<RelocateItem>,
    expires_at: Instant,
}

/// Single-use folder copy/move plans plus cancel flags for running ones.
/// Like delete plans, they live only in memory.
#[derive(Default)]
pub struct RelocatePlanRegistry {
    plans: Mutex<HashMap<String, RelocatePlan>>,
    running: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl RelocatePlanRegistry {
    pub fn insert(
        &self,
        connection_id: &str,
        bucket: &str,
        mode: RelocateMode,
        items: Vec<RelocateItem>,
    ) -> String {
        let id = uuid::Uuid::now_v7().to_string();
        let mut plans = self.plans.lock().expect("plan registry poisoned");
        let now = Instant::now();
        plans.retain(|_, p| p.expires_at > now);
        plans.insert(
            id.clone(),
            RelocatePlan {
                connection_id: connection_id.to_owned(),
                bucket: bucket.to_owned(),
                mode,
                items,
                expires_at: now + PLAN_TTL,
            },
        );
        id
    }

    /// Removes the plan and registers a cancel flag for its run.
    pub fn start(&self, id: &str) -> Result<(RelocatePlan, Arc<AtomicBool>), AppError> {
        let plan = self
            .plans
            .lock()
            .expect("plan registry poisoned")
            .remove(id)
            .filter(|p| p.expires_at > Instant::now())
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::InvalidInput,
                    "This confirmation has expired. Start the operation again.",
                )
            })?;
        let cancel = Arc::new(AtomicBool::new(false));
        self.running
            .lock()
            .expect("plan registry poisoned")
            .insert(id.to_owned(), cancel.clone());
        Ok((plan, cancel))
    }

    /// Asks a running copy/move to stop starting new objects.
    pub fn cancel(&self, id: &str) {
        if let Some(flag) = self.running.lock().expect("plan registry poisoned").get(id) {
            flag.store(true, Ordering::SeqCst);
        }
    }

    pub fn finish(&self, id: &str) {
        self.running
            .lock()
            .expect("plan registry poisoned")
            .remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_are_single_use_and_cancellable() {
        let registry = RelocatePlanRegistry::default();
        let id = registry.insert("c1", "b", RelocateMode::Move, Vec::new());

        let (plan, cancel) = registry.start(&id).unwrap();
        assert_eq!(plan.mode, RelocateMode::Move);
        assert!(registry.start(&id).is_err());

        registry.cancel(&id);
        assert!(cancel.load(Ordering::SeqCst));
        registry.finish(&id);
        registry.cancel(&id); // no-op once finished
    }
}
