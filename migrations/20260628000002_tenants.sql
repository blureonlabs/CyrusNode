-- Migration 002: Create the `tenants` table and seed the default operator tenant.
-- Every other table FKs to tenants(id). In V1 we run single-tenant, but the
-- column exists everywhere so multi-tenant is a config flip, not a schema change.

CREATE TABLE tenants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Seed: the operator's default tenant in V1.
INSERT INTO tenants (id, name)
VALUES ('00000000-0000-0000-0000-000000000001', 'apollo-default')
ON CONFLICT (id) DO NOTHING;
