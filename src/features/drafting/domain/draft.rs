//! Draft domain types. Pure data.

use serde::{Deserialize, Serialize};

/// Compact input for the email writer. The CLI / saga is responsible for
/// constructing this from research output — drafting domain does NOT import
/// research types (cross-feature import is forbidden).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftInput {
    /// One-sentence description of what the business sells.
    pub what_they_sell: String,
    /// 1–5 short value props (≤ 100 chars each).
    pub value_props: Vec<String>,
    /// 1–3 specific anchors the email should reference (named service, gap,
    /// recent post, etc.).
    pub anchors: Vec<String>,
    /// Industry-tone key (matches a file under `apollo/05-knowledge/industries/`).
    pub industry: Option<String>,
    /// First name (if known) for the salutation.
    pub contact_first_name: Option<String>,
    /// Operator's sign-off name.
    pub operator_signature: String,
}

/// A drafted outreach email — subject + body + the anchors the writer
/// actually used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailDraft {
    pub subject: String,
    pub body: String,
    pub personalization_anchors: Vec<String>,
}
