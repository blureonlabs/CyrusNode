# Event-Driven Design

> Apollo's modules talk through events, not function calls. This file is the rulebook.

---

## 1. Why events

- **Decoupling.** New agents subscribe to existing events without touching old code.
- **Replayability.** The events table is an immutable log; we can re-derive any artifact by replaying events from a checkpoint.
- **Observability.** Every meaningful state change is recorded with a `correlation_id`. Tracing follows.
- **Sagas.** Multi-step workflows (fan-out / fan-in) live in declarative coordinators, not nested function calls.

---

## 2. Event shape

```json
{
  "id": "01HK9...",            // ULID, time-sortable
  "tenant_id": "...",
  "type": "crawl.completed",   // dotted, lowercase
  "version": 1,                // schema version of payload
  "correlation_id": "...",     // ties together one pipeline run
  "causation_id": "...",       // the event that caused this one (null for roots)
  "company_id": "...",
  "occurred_at": "2026-06-27T08:14:01Z",
  "payload": { ... }           // type-specific, validated against JSON Schema
}
```

Rules:

- **Append-only.** Events are never updated or deleted. Corrections are new events.
- **Schema-validated.** Each `(type, version)` has a JSON Schema in `apollo/06-api/event-schemas/`.
- **Transactional.** An event is inserted in the same DB transaction as the state change that produced it. If the transaction rolls back, no one ever sees the event.

---

## 3. Event catalogue (V1)

Grouped by phase. Full schemas in [[../06-api/events]].

### Ingestion
- `url.submitted` — operator dropped a URL.
- `company.created` — a `companies` row was created (idempotent on URL).

### Crawl
- `crawl.requested`
- `crawl.completed` (or `crawl.failed` with `reason`)

### Extraction
- `extraction.completed`
- `extraction.empty`

### Analysis (parallel)
- `seo.completed`
- `tech.completed`
- `bizintel.completed`
- `social.completed` (V2)

### Synthesis
- `opportunities.generated`
- `roi.completed`

### Drafting (parallel)
- `email.drafted`
- `whatsapp.drafted`
- `proposal.drafted`

### Dossier
- `dossier.ready` — emitted by the saga when all three drafts exist.

### Operator
- `review.approved` / `review.rejected` / `review.replay_requested`

### Outbound
- `email.send_requested`
- `email.sent`
- `email.bounced` / `email.complained`
- `whatsapp.send_requested`
- `whatsapp.sent`

### Inbound
- `reply.received`
- `meeting.booked`

### Outcome
- `deal.outcome_recorded` (won | lost | stalled, with reason)

---

## 4. Sagas (fan-out / fan-in)

A saga is a small state machine that listens for a set of events and emits a derived event when its condition is met.

Example: the **Analysis Saga** waits for `seo.completed` + `tech.completed` + `bizintel.completed` for the same `company_id`, then emits `analysis.completed` and enqueues the Opportunity Finder job.

Declarative form (Rust):

```rust
saga! {
    name: "analysis",
    when_all: ["seo.completed", "tech.completed", "bizintel.completed"],
    correlation: company_id,
    emit: "analysis.completed",
    on_emit: enqueue_job("opportunity-finder", company_id),
    timeout: 10.minutes,
    on_timeout: emit("analysis.timed_out"),
}
```

Sagas live in `crates/apollo-events/src/sagas/`. Each saga has its own state row keyed by `(saga_name, correlation_id)`.

Two sagas in V1:
1. `analysis` — fan-in over the three analysis agents.
2. `drafting` — fan-in over the three draft agents, emits `dossier.ready`.

---

## 5. Event bus implementation

V1: **Postgres LISTEN/NOTIFY** plus a poll fallback.

- Writers `INSERT` into `events` and `NOTIFY events` in the same transaction.
- Workers `LISTEN events`. On notification, they query the events they care about (filtered by type subscription) from their last checkpoint.
- A 5-second poll catches notifications missed during a worker reconnect.

Why not Redis Streams / NATS / Kafka in V1? Postgres LISTEN/NOTIFY is one component to operate. We promote to Redis Streams when sustained event rate > 50/s, which is well above V1 volumes (probably 1–5/s on average).

V2 migration plan in [[../DECISIONS]] ADR-005.

---

## 6. Idempotency

Every event handler must be idempotent. The two patterns we accept:

1. **Conditional enqueue.** Before enqueuing a job, check whether a job with the same `(agent_name, company_id, input_hash)` already exists in `queued | running | succeeded`. If so, skip.
2. **Application-level dedup.** Each handler records the IDs of events it has processed in a `processed_events` table; double-delivery is a no-op.

`apollo-events` provides both as helpers. Saga handlers use pattern 1 by default.

---

## 7. Correlation and causation

- `correlation_id` is created at the **root** of a pipeline (typically `url.submitted`). Every downstream event carries the same id.
- `causation_id` is the immediate parent event id. Walking causation backwards reconstructs the dependency tree.

In tracing: the OpenTelemetry trace id == `correlation_id`. Span id == event id. This makes Tempo/Jaeger queries trivially correlated with DB events.

---

## 8. Event versioning

When a payload changes shape:

1. Bump `version` for that type in the schema directory.
2. Write an upcaster: `fn upcast_v1_to_v2(payload: V1) -> V2` lives in `apollo-events::upcasters`.
3. Handlers always operate on the *latest* version; the dispatcher upcasts at read time.

We never break old events. The events table is a historical record.

---

## 9. What NOT to do

- Do not `INSERT` an event outside a database transaction. You will get partial state.
- Do not emit an event from a handler before the transaction commits. Use Postgres `NOTIFY` (which fires at commit) — never an in-process callback that runs before commit.
- Do not put rich logic in sagas. Sagas only coordinate; the work happens in agents.
- Do not skip the schema. An event without a schema in the directory is a deploy blocker.

---

Links: [[overview]] · [[queue-system]] · [[database]] · [[../06-api/events]] · [[../DECISIONS]]
