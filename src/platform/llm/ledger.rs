//! Append-only file ledger for LLM calls. CLAUDE.md §6 mandates this.
//!
//! V1 writes to `out/llm_calls.jsonl`. V2 will mirror to the `llm_calls`
//! Postgres table once the worker pipeline is event-driven.
//!
//! The writer is intentionally tolerant: callers fire-and-forget via
//! [`LedgerWriter::record`] and treat any ledger error as best-effort. We
//! would rather lose a row than fail the LLM call because disk was full.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

/// A single LLM call observation, the unit the ledger writes one per line.
///
/// Fields mirror what we will eventually columnize in the `llm_calls`
/// Postgres table. Adding optional fields is non-breaking — old JSONL lines
/// will still deserialize because every new field gets `#[serde(default)]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCallRecord {
    /// Wall-clock UTC timestamp when the call completed.
    pub occurred_at: chrono::DateTime<chrono::Utc>,
    /// Provider-specific model identifier echoed from the request.
    pub model: String,
    /// Logical agent name (e.g. `"bizintel"`). Absent when the call did
    /// not flow through an agent (ad-hoc CLI use).
    #[serde(default)]
    pub agent_name: Option<String>,
    /// Prompt name from the prompt frontmatter.
    #[serde(default)]
    pub prompt_name: Option<String>,
    /// Prompt version from the prompt frontmatter.
    #[serde(default)]
    pub prompt_version: Option<u32>,
    /// Tenant / company id this call is attributed to (stringified UUID).
    #[serde(default)]
    pub company_id: Option<String>,
    /// Prompt token count reported by the provider.
    pub input_tokens: u32,
    /// Completion token count reported by the provider.
    pub output_tokens: u32,
    /// USD cost computed via [`super::cost::cost_usd_per_call`].
    pub cost_usd: f64,
    /// Wall-clock latency measured client-side, in milliseconds.
    pub latency_ms: u32,
    /// Whether the call returned a successful response.
    pub succeeded: bool,
    /// Stringified error, present only when `succeeded == false`.
    #[serde(default)]
    pub error: Option<String>,
}

/// Append-only ledger writer. Cheap to clone — wraps an Arc'd Mutex over the file.
#[derive(Clone)]
pub struct LedgerWriter {
    inner: Arc<Mutex<tokio::fs::File>>,
    path: PathBuf,
}

impl LedgerWriter {
    /// Open (or create) the ledger file at `path`. Creates parent dirs.
    ///
    /// Errors:
    /// - parent directory cannot be created
    /// - the file cannot be opened in append mode
    pub async fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        Ok(Self {
            inner: Arc::new(Mutex::new(file)),
            path,
        })
    }

    /// Open the ledger from a synchronous context. Used by constructors
    /// like `GeminiClient::from_env` that can't `.await`. Internally relies
    /// on `std::fs` to create the file, then promotes it into a Tokio
    /// `File` handle for async writes.
    pub fn open_blocking(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let std_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let file = tokio::fs::File::from_std(std_file);
        Ok(Self {
            inner: Arc::new(Mutex::new(file)),
            path,
        })
    }

    /// Path the ledger is writing to. Useful for diagnostics + the
    /// `apollo cost` subcommand.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one record. Newline-delimited JSON. Best-effort on flush.
    ///
    /// Errors:
    /// - record cannot be serialized (should be impossible given the type)
    /// - the underlying write fails
    pub async fn record(&self, rec: &LlmCallRecord) -> anyhow::Result<()> {
        let line = serde_json::to_string(rec)? + "\n";
        let mut guard = self.inner.lock().await;
        guard.write_all(line.as_bytes()).await?;
        guard.flush().await?;
        Ok(())
    }
}

/// A no-op writer used when no ledger is configured (tests, `--no-ledger`).
/// Has the same shape as [`LedgerWriter`] so call sites can swap easily.
#[derive(Clone, Default)]
pub struct NoopLedger;

impl NoopLedger {
    /// Always succeeds. Records nothing.
    pub async fn record(&self, _: &LlmCallRecord) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write three records, read them back via JSONL parse, assert order + content.
    /// Avoids `tempfile` — uses `std::env::temp_dir()` + a UUID-suffixed file.
    #[tokio::test]
    async fn appends_and_reads_back_in_order() {
        let path =
            std::env::temp_dir().join(format!("apollo-ledger-test-{}.jsonl", uuid::Uuid::new_v4()));
        let writer = LedgerWriter::open(&path).await.expect("open ledger");

        let make = |model: &str, input: u32| LlmCallRecord {
            occurred_at: chrono::Utc::now(),
            model: model.to_string(),
            agent_name: Some("bizintel".into()),
            prompt_name: Some("business-summary".into()),
            prompt_version: Some(1),
            company_id: Some("00000000-0000-0000-0000-000000000000".into()),
            input_tokens: input,
            output_tokens: 100,
            cost_usd: 0.0001,
            latency_ms: 250,
            succeeded: true,
            error: None,
        };

        let rec1 = make("gemini-2.5-flash", 100);
        let rec2 = make("gemini-2.5-flash", 200);
        let rec3 = make("claude-haiku-4-5", 300);

        writer.record(&rec1).await.expect("rec1");
        writer.record(&rec2).await.expect("rec2");
        writer.record(&rec3).await.expect("rec3");

        let body = tokio::fs::read_to_string(&path).await.expect("read back");
        let parsed: Vec<LlmCallRecord> = body
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).expect("parse jsonl"))
            .collect();

        assert_eq!(parsed.len(), 3, "expected 3 records");
        assert_eq!(parsed[0].input_tokens, 100);
        assert_eq!(parsed[1].input_tokens, 200);
        assert_eq!(parsed[2].model, "claude-haiku-4-5");
        assert_eq!(parsed[0].agent_name.as_deref(), Some("bizintel"));

        // Cleanup — ignore errors, the temp dir gets reaped anyway.
        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn records_error_path() {
        let path = std::env::temp_dir().join(format!(
            "apollo-ledger-test-err-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        let writer = LedgerWriter::open(&path).await.expect("open ledger");

        let rec = LlmCallRecord {
            occurred_at: chrono::Utc::now(),
            model: "gemini-2.5-flash".into(),
            agent_name: Some("bizintel".into()),
            prompt_name: None,
            prompt_version: None,
            company_id: None,
            input_tokens: 0,
            output_tokens: 0,
            cost_usd: 0.0,
            latency_ms: 12,
            succeeded: false,
            error: Some("rate limited (429)".into()),
        };
        writer.record(&rec).await.expect("record err");

        let body = tokio::fs::read_to_string(&path).await.expect("read back");
        let parsed: LlmCallRecord = serde_json::from_str(body.trim()).expect("parse");
        assert!(!parsed.succeeded);
        assert_eq!(parsed.error.as_deref(), Some("rate limited (429)"));

        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn noop_ledger_compiles_and_succeeds() {
        let noop = NoopLedger;
        let rec = LlmCallRecord {
            occurred_at: chrono::Utc::now(),
            model: "x".into(),
            agent_name: None,
            prompt_name: None,
            prompt_version: None,
            company_id: None,
            input_tokens: 0,
            output_tokens: 0,
            cost_usd: 0.0,
            latency_ms: 0,
            succeeded: true,
            error: None,
        };
        noop.record(&rec).await.expect("noop never fails");
    }
}
