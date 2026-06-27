use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Tenant identifier — every primary record carries one. V1 single-tenant; the
/// shape is preserved for V2 SaaS (ADR-007).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TenantId(pub Uuid);

/// Stable identifier for a company under research.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CompanyId(pub Uuid);

/// Correlation id ties together every event and span in one pipeline run.
/// Time-sortable (ULID).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CorrelationId(pub String);

impl CorrelationId {
    pub fn new() -> Self {
        Self(ulid::Ulid::new().to_string())
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}
