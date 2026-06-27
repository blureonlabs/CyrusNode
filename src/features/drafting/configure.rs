//! Wiring file for the drafting feature.

use std::sync::Arc;

use crate::features::drafting::application::DraftOutreach;
use crate::features::drafting::domain::DraftPort;
use crate::features::drafting::infra::EmailWriterAgent;
use crate::platform::events::EventSubscription;
use crate::platform::llm::LlmClient;
use crate::platform::prompts::PromptLoader;

#[derive(Clone)]
pub struct DraftingDeps {
    pub llm: Arc<dyn LlmClient>,
    pub prompts: PromptLoader,
    /// Path to the banned-phrases file (one phrase per line, case-insensitive).
    pub banned_phrases_path: std::path::PathBuf,
}

pub struct DraftingModule {
    pub routes: axum::Router<()>,
    pub subscriptions: Vec<EventSubscription>,
    pub draft_outreach: Arc<DraftOutreach>,
}

/// Wires the four boxes for the drafting feature.
pub fn configure(deps: DraftingDeps) -> DraftingModule {
    let draft: Arc<dyn DraftPort> = Arc::new(EmailWriterAgent::new(
        deps.llm.clone(),
        deps.prompts.clone(),
        deps.banned_phrases_path.clone(),
    ));

    let draft_outreach = Arc::new(DraftOutreach::new(draft));

    DraftingModule {
        routes: super::presentation::routes(),
        subscriptions: super::presentation::subscriptions(),
        draft_outreach,
    }
}
