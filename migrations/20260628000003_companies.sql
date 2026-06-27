-- Migration 003: Create the `companies` table.
-- Companies are the central entity Apollo researches and engages.
-- Soft-deleted via deleted_at; the partial indexes exclude soft-deleted rows
-- so hot-path queries stay fast. The (tenant_id, host) unique constraint is
-- our de-duplication key for inbound discovery.

CREATE TABLE companies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    url TEXT NOT NULL,                    -- normalized
    host TEXT NOT NULL,                   -- e.g. abcdental.ae
    name TEXT,                            -- discovered name
    industry TEXT,                        -- mapped to playbook key
    country TEXT,
    city TEXT,
    status TEXT NOT NULL DEFAULT 'discovered',
        -- discovered | researching | researched | outreach | engaged | won | lost
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_researched_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (tenant_id, host)
);

CREATE INDEX idx_companies_status ON companies(tenant_id, status) WHERE deleted_at IS NULL;
CREATE INDEX idx_companies_industry ON companies(tenant_id, industry) WHERE deleted_at IS NULL;
