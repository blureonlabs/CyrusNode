//! Draft domain types. Pure data.

use serde::{Deserialize, Serialize};

/// A drafted outreach email — subject + body. Personalization details land in
/// later sprints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailDraft {
    pub subject: String,
    pub body: String,
}
