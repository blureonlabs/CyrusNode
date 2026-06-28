//! Pure data types the operator sees while reviewing dossiers.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// What the operator sees on each iteration. Self-contained — the review
/// feature does not import research or drafting types (the file persistence
/// layer constructs `QueueItem` from the on-disk dossier JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    /// Stable identifier of the company being reviewed.
    pub company_id: Uuid,
    /// Canonical URL of the company's site (used as the operator anchor).
    pub url: String,
    /// Optional industry tag carried over from the dossier.
    pub industry: Option<String>,
    /// One-line summary the operator skims first (BusinessSummary.what_they_sell).
    pub summary_line: String,
    /// Subject of the drafted email.
    pub email_subject: String,
    /// Body of the drafted email.
    pub email_body: String,
    /// Personalization anchors the writer claims it used.
    pub email_anchors: Vec<String>,
    /// Optional recipient address. When present, [`Decision::ApproveAndSend`]
    /// can mail it; otherwise we fall back to writing an outbox file.
    pub recipient_email: Option<String>,
}

/// The set of outcomes an operator can choose for a queue item.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Approve the draft and dispatch it through the configured sender.
    ApproveAndSend,
    /// No recipient address — write to disk for manual send.
    ApproveToOutbox,
    /// Open the body in `$EDITOR` and re-prompt.
    Edit,
    /// Reject the draft (moved to the rejected directory).
    Reject,
    /// Leave the item in place and move to the next one.
    Skip,
    /// Abort the review session.
    Quit,
}
