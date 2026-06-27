-- Migration 004: Create the `contacts` table.
-- Contacts are people associated with a company (decision makers, gatekeepers).
-- The unique index on lower(email) is partial because not every contact has
-- an email (some are LinkedIn-only or phone-only).

CREATE TABLE contacts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    company_id UUID NOT NULL REFERENCES companies(id),
    name TEXT,
    role TEXT,
    email TEXT,
    phone TEXT,
    whatsapp TEXT,
    linkedin_url TEXT,
    email_status TEXT,                    -- valid | bounced | complained | unknown
    source TEXT NOT NULL,                 -- website | linkedin | gmaps | manual
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_contacts_company ON contacts(company_id);
CREATE UNIQUE INDEX idx_contacts_email ON contacts(tenant_id, lower(email)) WHERE email IS NOT NULL;
