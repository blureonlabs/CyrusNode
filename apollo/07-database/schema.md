# Schema

Canonical DDL lives in [[../02-architecture/database]] §2. This page is the bird's-eye summary so an engineer can orient before reading SQL.

## Entities

```
tenants ──< companies ──< contacts
              │
              ├──< conversations ──< messages
              │
              ├──< sequences ──< sequence_steps
              │
              ├──< deals
              │
              └──< artifacts (one per (agent, version, input_hash))

jobs (queue) ─── correlation_id ───► events (log)

llm_calls (audit) ─── correlation_id ───► events

saga_state, processed_events, prompt_versions  (operational tables)
```

## Conventions
- UUID primary keys (gen_random_uuid) — except `events.id` which is ULID TEXT.
- All primary entities have `tenant_id`. Repository queries always include it.
- Append-only for `events`, `llm_calls`, terminal `jobs`, `artifacts`.
- Soft-delete via `deleted_at` on `companies`, `conversations`, `deals`.

## Cache keys
The `artifacts` table's `(tenant_id, agent_name, agent_version, input_hash)` unique constraint is the system's cache. Re-running a pipeline never re-pays for work that already produced the same output.

## Source of truth vs derived
- **Source**: Postgres tables above.
- **Derived**: Qdrant collections (`past_wins`, `objections`, `proposals`), materialized views (`cost_per_company_daily`), Elasticsearch (V3, optional).

Links: [[er-diagram]] · [[../02-architecture/database]]
