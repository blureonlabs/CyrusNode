//! Bootstrap — the only file that knows about all features.
//!
//! Builds platform handles once, calls each feature's `configure(deps)`, returns
//! a `Bootstrap` struct the binaries use to construct their entry points.

use std::sync::Arc;

use anyhow::Result;
use axum::Router;

use crate::features::drafting::{self, DraftingDeps};
use crate::features::research::{self, ResearchDeps};
use crate::platform::db::{build_pool, DbConfig, PgPool};
use crate::platform::events::EventSubscription;
use crate::platform::knowledge::PlaybookLoader;
use crate::platform::llm::{GeminiClient, LlmClient};
use crate::platform::prompts::PromptLoader;

/// Handles to every cross-cutting capability. Built once at boot; cloned freely.
#[derive(Clone)]
pub struct PlatformDeps {
    pub db: PgPool,
}

/// Aggregated routes + subscriptions from every feature.
pub struct Bootstrap {
    pub router: Router<()>,
    pub subscriptions: Vec<EventSubscription>,
    pub deps: PlatformDeps,
}

/// Wire everything for the API + worker binaries (requires Supabase + Gemini key).
///
/// Reads from the environment via [`DbConfig::from_env`] and
/// [`GeminiClient::from_env`], builds the connection pool, then constructs each
/// feature module. Gemini is the sole LLM provider per ADR-012.
pub async fn build() -> Result<Bootstrap> {
    let cfg = DbConfig::from_env()?;
    let db = build_pool(&cfg).await?;
    let deps = PlatformDeps { db };

    let prompts = PromptLoader::load("apollo/04-prompts").await?;
    let playbooks = PlaybookLoader::load("apollo/05-knowledge/industries").await?;
    let gemini: Arc<dyn LlmClient> = Arc::new(GeminiClient::from_env()?);

    let research = research::configure(ResearchDeps {
        llm: gemini.clone(),
        prompts: prompts.clone(),
        playbooks: playbooks.clone(),
    });
    let drafting = drafting::configure(DraftingDeps {
        llm: gemini.clone(),
        prompts: prompts.clone(),
        playbooks: playbooks.clone(),
        banned_phrases_path: std::path::PathBuf::from(
            "apollo/05-knowledge/banned-email-phrases.txt",
        ),
    });

    let router = Router::new().merge(research.routes).merge(drafting.routes);

    let mut subscriptions = Vec::new();
    subscriptions.extend(research.subscriptions);
    subscriptions.extend(drafting.subscriptions);

    Ok(Bootstrap {
        router,
        subscriptions,
        deps,
    })
}
