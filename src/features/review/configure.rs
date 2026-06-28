//! Wiring file for the review feature.
//!
//! Single matchmaker for the four boxes. Builds adapters from cross-cutting
//! handles supplied by the caller (typically `bootstrap.rs` or the CLI).

use std::path::PathBuf;
use std::sync::Arc;

use crate::features::review::application::RunReview;
use crate::features::review::domain::{OperatorPrompt, QueueRepoPort, SendPort};
use crate::features::review::infra::{FilesystemQueueRepo, StdinPrompter};
use crate::platform::events::EventSubscription;

/// Cross-cutting handles required to construct the review feature.
#[derive(Clone)]
pub struct ReviewDeps {
    /// Email/outbound transport used when the operator approves a draft.
    pub sender: Arc<dyn SendPort>,
    /// Address presented as the email `From:` header.
    pub from_address: String,
    /// Root directory containing `dossiers/`, `sent/`, `outbox/`, `rejected/`.
    pub out_root: PathBuf,
}

/// Public surface of the review feature after wiring.
pub struct ReviewModule {
    /// HTTP routes contributed by the feature (none in V1).
    pub routes: axum::Router<()>,
    /// Event subscriptions contributed by the feature (none in V1).
    pub subscriptions: Vec<EventSubscription>,
    /// The CLI-driven review loop.
    pub run_review: Arc<RunReview>,
}

/// Wire the review feature.
pub fn configure(deps: ReviewDeps) -> ReviewModule {
    let queue: Arc<dyn QueueRepoPort> = Arc::new(FilesystemQueueRepo::new(deps.out_root));
    let prompter: Arc<dyn OperatorPrompt> = Arc::new(StdinPrompter);
    let run_review = Arc::new(RunReview::new(
        queue,
        prompter,
        deps.sender,
        deps.from_address,
    ));

    ReviewModule {
        routes: axum::Router::new(),
        subscriptions: Vec::new(),
        run_review,
    }
}
