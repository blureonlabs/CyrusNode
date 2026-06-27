# Database

> Postgres is the source of truth. Qdrant is a derived store. If they disagree, Postgres wins.

---

## 1. Conventions

- **All tables** have: `id UUID PRIMARY KEY DEFAULT gen_random_uuid()`, `tenant_id UUID NOT NULL`, `created_at TIMESTAMPTZ NOT NULL DEFAULT now()`, `updated_at TIMESTAMPTZ NOT NULL DEFAULT now()`.
- **Soft delete**: `deleted_at TIMESTAMPTZ NULL` instead of `DELETE` for entities that may be referenced historically (companies, conversations, deals).
- **Append-only tables** (events, jobs once terminal, llm_calls) are never updated. Mutations create new rows.
- **Time** is `TIMESTAMPTZ` in UTC. No `TIMESTAMP WITHOUT TIME ZONE` anywhere.
- **Strings** that are URLs are stored normalized (lowercase host, no fragment, UTM stripped).
- **IDs in events / jobs payloads** are ULIDs as TEXT for time-sortable readability; primary keys remain UUID for compatibility.
- **Indexes** are explicit in migration files. No "create indexes later" — every query that ships with code ships with its index.

---

## 2. Core tables

### `tenants`
```sql
CREATE TABLE tenants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Seeded: the operator's tenant in V1.
```

### `companies`
```sql
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
```

### `contacts`
```sql
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
```

### `jobs` (the queue)
```sql
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
```

Queue semantics in [[queue-system]].

### `events`
```sql
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
```

### `artifacts` (agent outputs)
```sql
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
```

The uniqueness constraint is the **cache key**. A second call with the same `input_hash` reads the existing row instead of re-computing.

### `llm_calls`
```sql
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
```

### `conversations`
```sql
CREATE TABLE conversations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    company_id UUID NOT NULL REFERENCES companies(id),
    contact_id UUID REFERENCES contacts(id),
    channel TEXT NOT NULL,                -- email | whatsapp | linkedin | phone
    status TEXT NOT NULL DEFAULT 'open',  -- open | meeting_booked | won | lost | dormant
    last_message_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_conversations_company ON conversations(company_id);
CREATE INDEX idx_conversations_status ON conversations(tenant_id, status, last_message_at);
```

### `messages`
```sql
CREATE TABLE messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    conversation_id UUID NOT NULL REFERENCES conversations(id),
    direction TEXT NOT NULL,              -- outbound | inbound
    channel TEXT NOT NULL,
    subject TEXT,
    body TEXT NOT NULL,
    body_html TEXT,
    external_id TEXT,                     -- Resend message id, etc.
    in_reply_to TEXT,                     -- for email threading
    artifact_id UUID REFERENCES artifacts(id), -- which draft produced this
    status TEXT NOT NULL DEFAULT 'pending',
        -- pending | sent | delivered | bounced | failed | received
    sent_at TIMESTAMPTZ,
    received_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_messages_conversation ON messages(conversation_id, created_at);
CREATE INDEX idx_messages_external ON messages(external_id) WHERE external_id IS NOT NULL;
```

### `sequences` and `sequence_steps`
```sql
CREATE TABLE sequences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    company_id UUID NOT NULL REFERENCES companies(id),
    playbook_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active', -- active | paused | abandoned | converted
    current_step INT NOT NULL DEFAULT 0,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    next_action_at TIMESTAMPTZ
);

CREATE TABLE sequence_steps (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sequence_id UUID NOT NULL REFERENCES sequences(id),
    step_index INT NOT NULL,
    channel TEXT NOT NULL,
    day_offset INT NOT NULL,
    artifact_id UUID REFERENCES artifacts(id),
    message_id UUID REFERENCES messages(id),
    status TEXT NOT NULL DEFAULT 'pending',
    scheduled_for TIMESTAMPTZ,
    UNIQUE (sequence_id, step_index)
);
```

### `deals`
```sql
CREATE TABLE deals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    company_id UUID NOT NULL REFERENCES companies(id),
    stage TEXT NOT NULL DEFAULT 'qualified',
        -- qualified | meeting_booked | proposal_sent | negotiating | won | lost
    amount_usd NUMERIC(12,2),
    expected_close_date DATE,
    won_reason TEXT,
    lost_reason TEXT,
    closed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### `saga_state`
```sql
CREATE TABLE saga_state (
    saga_name TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    received JSONB NOT NULL DEFAULT '{}'::jsonb,  -- map: event_type -> event_id
    completed_at TIMESTAMPTZ,
    timed_out_at TIMESTAMPTZ,
    PRIMARY KEY (saga_name, correlation_id)
);
```

### `processed_events`
```sql
CREATE TABLE processed_events (
    handler TEXT NOT NULL,
    event_id TEXT NOT NULL,
    processed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (handler, event_id)
);
```

### `prompt_versions`
```sql
CREATE TABLE prompt_versions (
    name TEXT NOT NULL,
    version INT NOT NULL,
    body_sha256 TEXT NOT NULL,
    frontmatter JSONB NOT NULL,
    loaded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (name, version)
);
```

On startup, the prompt loader inserts a row per (name, version) it sees. Hot reloads check if the body hash changed without a version bump and refuse to serve — forcing the operator to bump the version.

---

## 3. Materialized views (V1)

### `cost_per_company_daily`
```sql
CREATE MATERIALIZED VIEW cost_per_company_daily AS
SELECT
    tenant_id,
    company_id,
    date_trunc('day', occurred_at) AS day,
    COUNT(*) AS calls,
    SUM(input_tokens) AS input_tokens,
    SUM(output_tokens) AS output_tokens,
    SUM(cost_usd) AS cost_usd
FROM llm_calls
GROUP BY 1,2,3;

CREATE UNIQUE INDEX ON cost_per_company_daily(tenant_id, company_id, day);
```

Refreshed every 5 minutes by a worker task.

### `pipeline_health`
```sql
CREATE VIEW pipeline_health AS
SELECT
    tenant_id,
    agent_name,
    COUNT(*) FILTER (WHERE status = 'queued')    AS queued,
    COUNT(*) FILTER (WHERE status = 'leased')    AS leased,
    COUNT(*) FILTER (WHERE status = 'running')   AS running,
    COUNT(*) FILTER (WHERE status = 'failed' AND finished_at > now() - interval '1 hour') AS recent_failures
FROM jobs
GROUP BY 1,2;
```

---

## 4. Qdrant collections

| Collection | Vector | Payload |
|---|---|---|
| `past_wins` | embedding of (industry + opportunity + outcome_summary) | `{ company_id, opportunity_name, deal_amount, outcome, closed_at }` |
| `objections` | embedding of objection text | `{ company_id, objection_text, response_used, worked: bool }` |
| `proposals` | embedding of (industry + opportunity bundle) | `{ company_id, proposal_id, won: bool }` |

Qdrant is populated by post-commit hooks: when a `deal.outcome_recorded` event fires, the embedding pipeline upserts to `past_wins`. Qdrant is a derived store — rebuildable from Postgres in minutes.

---

## 5. Migration policy

- Migrations live in `migrations/` as `<timestamp>_<slug>.sql`. Forward-only. We do not write `DOWN` migrations after V1 launch.
- Every PR that adds a column adds a backfill plan in the PR description, even if the backfill is "default null is fine."
- Schema breaks (renaming columns, changing types) require an ADR and a two-phase migration (add new → migrate → remove old).

---

## 6. Backup and DR

- Daily logical backups (`pg_dump`) shipped to S3-equivalent. 30-day retention.
- WAL archiving enabled from day one (V1 single Postgres → V2 managed Postgres on Render or Neon).
- Restore tested quarterly. Untested backups are not backups.

---

Links: [[overview]] · [[event-driven]] · [[queue-system]] · [[../07-database/schema]] · [[../07-database/er-diagram]]
