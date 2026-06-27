-- Migration 008: Create the `llm_calls` table (append-only LLM telemetry).
-- One row per provider call. cost_usd is NUMERIC(10,6) so per-call costs
-- down to a micro-dollar are exact (no float drift in roll-ups). Used for
-- cost-per-company dashboards and per-agent budget guardrails.

CREATE TABLE llm_calls (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    company_id UUID REFERENCES companies(id),
    correlation_id TEXT NOT NULL,
    agent_name TEXT NOT NULL,
    prompt_name TEXT NOT NULL,
    prompt_version INT NOT NULL,
    model TEXT NOT NULL,
    input_tokens INT NOT NULL,
    output_tokens INT NOT NULL,
    cost_usd NUMERIC(10,6) NOT NULL,
    latency_ms INT NOT NULL,
    schema_retry BOOLEAN NOT NULL DEFAULT false,
    succeeded BOOLEAN NOT NULL,
    error TEXT,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_llm_calls_company ON llm_calls(company_id, occurred_at);
CREATE INDEX idx_llm_calls_cost ON llm_calls(tenant_id, occurred_at);
CREATE INDEX idx_llm_calls_agent ON llm_calls(tenant_id, agent_name, occurred_at);
