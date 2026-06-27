//! Cost lookup table per model.
//!
//! Pricing source: ADR-010 + Anthropic and Google public pricing pages
//! (snapshot 2026-06). When a new model is added, update both the match
//! arm and ADR-010 in the same PR so the source of truth doesn't drift.
//!
//! All rates are USD per 1M tokens. We compute on the float boundary,
//! which is fine for the `NUMERIC(10,6)` column in `llm_calls`.

/// Compute USD cost given a model id and token counts.
///
/// Returns `0.0` for unknown models with a `tracing::warn!` so the call
/// still records — we'd rather have a row with zero cost than drop the
/// observation entirely. Reconciliation jobs can patch costs later if
/// pricing tables shift.
pub fn cost_usd_per_call(model: &str, input_tokens: u32, output_tokens: u32) -> f64 {
    // (input_per_1M, output_per_1M) in USD
    let (in_rate, out_rate) = match model {
        // Gemini
        "gemini-2.5-flash" | "models/gemini-2.5-flash" => (0.30, 2.50),
        "gemini-2.5-pro" | "models/gemini-2.5-pro" => (1.25, 10.00),
        // Anthropic
        "claude-haiku-4-5" | "claude-haiku-4-5-20251001" => (1.00, 5.00),
        "claude-sonnet-4-6" | "claude-sonnet-4-6-20251001" => (3.00, 15.00),
        "claude-opus-4-7" => (15.00, 75.00),
        other => {
            tracing::warn!(model = %other, "unknown model — cost_usd = 0");
            return 0.0;
        }
    };
    (input_tokens as f64 / 1_000_000.0) * in_rate + (output_tokens as f64 / 1_000_000.0) * out_rate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_flash_known_rate() {
        // 1M in + 1M out at $0.30 / $2.50 = $2.80
        let c = cost_usd_per_call("gemini-2.5-flash", 1_000_000, 1_000_000);
        assert!((c - 2.80).abs() < 1e-9, "got {c}");
    }

    #[test]
    fn anthropic_sonnet_known_rate() {
        // 1k in + 1k out at $3 / $15 per 1M = 0.003 + 0.015 = 0.018
        let c = cost_usd_per_call("claude-sonnet-4-6", 1_000, 1_000);
        assert!((c - 0.018).abs() < 1e-9, "got {c}");
    }

    #[test]
    fn unknown_model_returns_zero() {
        assert_eq!(cost_usd_per_call("gpt-9", 100, 100), 0.0);
    }

    #[test]
    fn zero_tokens_zero_cost() {
        assert_eq!(cost_usd_per_call("claude-haiku-4-5", 0, 0), 0.0);
    }
}
