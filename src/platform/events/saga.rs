//! Minimal saga runtime — fan-in over multiple event types per `correlation_id`.
//!
//! See `apollo/02-architecture/event-driven.md` §4. V1 sagas are in-memory:
//! when all `wait_for` types have arrived for the same `correlation_id`, the
//! coordinator fires `on_complete`. The Analysis Saga (fan-in over
//! `seo.completed` + `tech.completed` + `bizintel.completed`) and the
//! Drafting Saga (fan-in over the three draft agents emitting
//! `dossier.ready`) are wired on top of this primitive.
//!
//! TODO(v2): persist `saga_state` to Postgres so coordinators survive
//! restarts. The in-memory map below loses pending sagas on crash.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio::sync::Mutex;

use super::EventEnvelope;

/// Callback invoked when a saga completes for a given correlation id.
pub type SagaCallback = Arc<dyn Fn(String) + Send + Sync>;

/// A saga's declarative definition.
///
/// `on_complete` receives the `correlation_id` that completed and is
/// expected to do the side-effect (typically enqueue a follow-up job or
/// publish a derived event). The closure must be cheap and non-blocking;
/// long work should be off-loaded to a spawned task.
pub struct SagaDefinition {
    /// Stable, human-readable saga name, used as part of the state key.
    pub name: &'static str,
    /// Event types that must all have been observed for the same
    /// `correlation_id` before completion fires.
    pub wait_for: Vec<&'static str>,
    /// Callback invoked exactly once per completed `correlation_id`.
    pub on_complete: SagaCallback,
}

/// In-flight state for one `(saga_name, correlation_id)` pair.
#[derive(Default, Debug)]
pub struct SagaState {
    /// Event types observed so far.
    pub received: HashSet<&'static str>,
}

/// In-memory coordinator that drives the registered saga definitions.
///
/// Construction is cheap (no I/O). `observe` is the hot path — called once
/// per dispatched event by the listener layer.
pub struct SagaCoordinator {
    defs: Vec<SagaDefinition>,
    state: Mutex<HashMap<(&'static str, String), SagaState>>,
}

impl SagaCoordinator {
    /// Build a coordinator from the given definitions.
    pub fn new(defs: Vec<SagaDefinition>) -> Self {
        Self {
            defs,
            state: Mutex::new(HashMap::new()),
        }
    }

    /// Feed an event into every saga whose `wait_for` mentions its type.
    ///
    /// When a saga's required set is fully observed for a `correlation_id`:
    /// 1. The state entry is removed (so `on_complete` fires exactly once).
    /// 2. `on_complete` is invoked synchronously with the correlation id.
    ///
    /// `on_complete` is intentionally synchronous — wiring expects it to
    /// enqueue work, not perform it.
    pub async fn observe(&self, env: &EventEnvelope) {
        let event_type = env.event_type.as_str();
        let correlation_id = env.correlation_id.clone();

        let mut completions: Vec<(SagaCallback, String)> = Vec::new();

        {
            let mut state = self.state.lock().await;
            for def in &self.defs {
                // Match against the static slice so we get back the `'static`
                // str the state map keys on.
                let Some(matched_type) = def.wait_for.iter().find(|t| **t == event_type) else {
                    continue;
                };

                let key = (def.name, correlation_id.clone());
                let entry = state.entry(key.clone()).or_default();
                entry.received.insert(*matched_type);

                let all_received = def.wait_for.iter().all(|t| entry.received.contains(*t));
                if all_received {
                    state.remove(&key);
                    completions.push((def.on_complete.clone(), correlation_id.clone()));
                }
            }
        }

        for (cb, cid) in completions {
            cb(cid);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn make_envelope(event_type: &str, correlation_id: &str) -> EventEnvelope {
        EventEnvelope {
            id: "01HKTEST".to_string(),
            tenant_id: uuid::Uuid::nil(),
            event_type: event_type.to_string(),
            version: 1,
            correlation_id: correlation_id.to_string(),
            causation_id: None,
            company_id: None,
            payload: serde_json::json!({}),
            occurred_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn fires_on_complete_only_when_all_event_types_received() {
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();
        let def = SagaDefinition {
            name: "analysis",
            wait_for: vec!["seo.completed", "tech.completed", "bizintel.completed"],
            on_complete: Arc::new(move |_cid| {
                count_clone.fetch_add(1, Ordering::SeqCst);
            }),
        };
        let coord = SagaCoordinator::new(vec![def]);

        coord
            .observe(&make_envelope("seo.completed", "corr-1"))
            .await;
        assert_eq!(count.load(Ordering::SeqCst), 0);
        coord
            .observe(&make_envelope("tech.completed", "corr-1"))
            .await;
        assert_eq!(count.load(Ordering::SeqCst), 0);
        coord
            .observe(&make_envelope("bizintel.completed", "corr-1"))
            .await;
        assert_eq!(count.load(Ordering::SeqCst), 1);

        // Duplicate delivery after completion does not refire.
        coord
            .observe(&make_envelope("bizintel.completed", "corr-1"))
            .await;
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn isolates_state_per_correlation_id() {
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();
        let def = SagaDefinition {
            name: "drafting",
            wait_for: vec!["email.drafted", "whatsapp.drafted"],
            on_complete: Arc::new(move |_cid| {
                count_clone.fetch_add(1, Ordering::SeqCst);
            }),
        };
        let coord = SagaCoordinator::new(vec![def]);

        coord.observe(&make_envelope("email.drafted", "a")).await;
        coord.observe(&make_envelope("email.drafted", "b")).await;
        assert_eq!(count.load(Ordering::SeqCst), 0);
        coord.observe(&make_envelope("whatsapp.drafted", "a")).await;
        assert_eq!(count.load(Ordering::SeqCst), 1);
        coord.observe(&make_envelope("whatsapp.drafted", "b")).await;
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }
}
