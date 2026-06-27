-- Migration 007: Create the `artifacts` table (agent outputs).
-- The (tenant, agent, version, input_hash) unique constraint IS the cache key:
-- repeated agent calls with identical inputs read the existing row instead of
-- recomputing. This is how Apollo keeps LLM spend bounded.

CREATE TABLE artifacts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    company_id UUID NOT NULL REFERENCES companies(id),
    agent_name TEXT NOT NULL,
    agent_version INT NOT NULL,
    input_hash TEXT NOT NULL,
    prompt_name TEXT,                     -- nullable for deterministic agents
    prompt_version INT,
    payload JSONB NOT NULL,               -- the structured output
    payload_schema_version INT NOT NULL,
    bytes BIGINT NOT NULL,                -- size of payload, for cost analysis
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, agent_name, agent_version, input_hash)
);

CREATE INDEX idx_artifacts_company ON artifacts(company_id, agent_name);
