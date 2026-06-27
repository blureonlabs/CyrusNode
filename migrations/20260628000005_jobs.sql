-- Migration 005: Create the `jobs` table (the work queue).
-- Postgres-backed queue with lease semantics. The (tenant, agent, version,
-- input_hash) unique constraint gives us deduplication: re-enqueuing the same
-- work is a no-op. The partial indexes target the two hot reads: pickable
-- jobs and expired leases.

CREATE TABLE jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    agent_name TEXT NOT NULL,
    agent_version INT NOT NULL,
    company_id UUID REFERENCES companies(id),
    correlation_id TEXT NOT NULL,
    input JSONB NOT NULL,
    input_hash TEXT NOT NULL,             -- SHA256(canonical(input))
    status TEXT NOT NULL DEFAULT 'queued',
        -- queued | leased | running | succeeded | failed | cancelled
    priority INT NOT NULL DEFAULT 100,    -- lower = sooner
    available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    leased_until TIMESTAMPTZ,
    attempt_count INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 3,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    UNIQUE (tenant_id, agent_name, agent_version, input_hash)
);

CREATE INDEX idx_jobs_pickable ON jobs(status, priority, available_at)
    WHERE status = 'queued';
CREATE INDEX idx_jobs_expired_leases ON jobs(leased_until)
    WHERE status = 'leased';
CREATE INDEX idx_jobs_correlation ON jobs(correlation_id);
