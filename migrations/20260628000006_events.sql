-- Migration 006: Create the `events` table (append-only event log).
-- IDs are ULIDs stored as TEXT so they are both globally unique and
-- time-sortable. correlation_id stitches together a saga; causation_id
-- points to the event that caused this one.

CREATE TABLE events (
    id TEXT PRIMARY KEY,                  -- ULID
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    type TEXT NOT NULL,
    version INT NOT NULL,
    correlation_id TEXT NOT NULL,
    causation_id TEXT,
    company_id UUID REFERENCES companies(id),
    payload JSONB NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_events_correlation ON events(correlation_id, occurred_at);
CREATE INDEX idx_events_type ON events(tenant_id, type, occurred_at);
CREATE INDEX idx_events_company ON events(company_id, occurred_at) WHERE company_id IS NOT NULL;
