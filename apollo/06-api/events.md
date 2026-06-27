# Events — Catalogue & Payloads

> The full V1 event catalogue. Each entry: type, version, payload shape, who emits, who consumes.

The conceptual model is in [[../02-architecture/event-driven]]. This file is the contract.

---

## 1. Format

Every event payload is a JSON object validated against a JSON Schema in `apollo/06-api/event-schemas/<type>.v<n>.json`.

Common envelope (already in [[../02-architecture/database]] → `events` table):

```
{ id, tenant_id, type, version, correlation_id, causation_id, company_id, payload, occurred_at }
```

---

## 2. Ingestion

### `url.submitted` v1
- **Emitted by**: `apollo-api` on `POST /companies`.
- **Consumed by**: company saga (creates `companies` row).
- **Payload**:
  ```json
  { "url": "https://abcdental.ae", "submitted_by": "operator", "source": "manual" }
  ```

### `company.created` v1
- **Emitted by**: company saga.
- **Consumed by**: crawler enqueuer.
- **Payload**:
  ```json
  { "company_id": "uuid", "url": "...", "host": "abcdental.ae" }
  ```

---

## 3. Crawl

### `crawl.requested` v1
- **Emitted by**: company saga.
- **Consumed by**: crawler worker (via job).
- **Payload**: `{ "company_id": "uuid", "url": "...", "use_headless": false }`

### `crawl.completed` v1
- **Emitted by**: crawler.
- **Consumed by**: extractor enqueuer.
- **Payload**: `{ "company_id": "uuid", "artifact_id": "uuid", "pages_fetched": 18 }`

### `crawl.blocked` v1
- **Emitted by**: crawler.
- **Consumed by**: operator notification.
- **Payload**: `{ "company_id": "uuid", "reason": "robots_disallowed | cf_block | js_required" }`

### `crawl.unreachable` v1
- **Emitted by**: crawler.
- **Payload**: `{ "company_id": "uuid", "reason": "dns | tls | 4xx | 5xx", "detail": "..." }`

---

## 4. Extraction

### `extraction.completed` v1
- **Payload**: `{ "company_id": "uuid", "artifact_id": "uuid", "word_count": 4810 }`

### `extraction.empty` v1
- **Payload**: `{ "company_id": "uuid", "word_count": 0, "headless_attempted": false }`
- The crawler saga re-enqueues with `use_headless: true` if not yet tried.

---

## 5. Analysis (parallel after extraction)

### `seo.completed` v1
- **Payload**: `{ "company_id": "uuid", "artifact_id": "uuid", "lighthouse_perf": 62 }`

### `tech.completed` v1
- **Payload**: `{ "company_id": "uuid", "artifact_id": "uuid", "stack_signal_count": 9 }`

### `bizintel.completed` v1
- **Payload**: `{ "company_id": "uuid", "artifact_id": "uuid", "confidence": 0.86 }`

### `bizintel.insufficient_context` v1
- **Payload**: `{ "company_id": "uuid", "reason": "extraction_too_thin" }`

### `social.completed` v1 *(V2)*
- **Payload**: `{ "company_id": "uuid", "artifact_id": "uuid", "follower_count": 4200 }`

### Saga: `analysis.completed` v1
- **Emitted by**: Analysis Saga when seo + tech + bizintel all received for same `correlation_id`.
- **Consumed by**: Opportunity Finder enqueuer.

---

## 6. Synthesis

### `opportunities.generated` v1
- **Payload**: `{ "company_id": "uuid", "artifact_id": "uuid", "count": 5 }`

### `roi.completed` v1
- **Payload**: `{ "company_id": "uuid", "artifact_id": "uuid" }`

---

## 7. Drafting (parallel)

### `email.drafted` v1
- **Payload**: `{ "company_id": "uuid", "artifact_id": "uuid", "word_count": 96 }`

### `whatsapp.drafted` v1
- **Payload**: `{ "company_id": "uuid", "artifact_id": "uuid", "word_count": 34 }`

### `proposal.drafted` v1
- **Payload**: `{ "company_id": "uuid", "artifact_id": "uuid" }`

### Saga: `dossier.ready` v1
- **Emitted by**: Drafting Saga when all three drafts received.
- **Consumed by**: dashboard review queue.

---

## 8. Operator

### `review.approved` v1
- **Payload**:
  ```json
  {
    "company_id": "uuid",
    "approved_drafts": ["email", "whatsapp"],
    "edits": { "email": "operator-edited body..." }
  }
  ```

### `review.rejected` v1
- **Payload**: `{ "company_id": "uuid", "reason": "wrong_business_classification" }`

### `review.replay_requested` v1
- **Payload**: `{ "company_id": "uuid", "agents": ["bizintel", "email-writer"], "hint": "they're a manufacturer, not retail" }`

---

## 9. Outbound

### `email.send_requested` v1
- **Payload**: `{ "company_id": "uuid", "message_id": "uuid" }`

### `email.sent` v1
- **Payload**: `{ "company_id": "uuid", "message_id": "uuid", "resend_id": "..." }`

### `email.delivered` v1 / `email.bounced` v1 / `email.complained` v1
- **Payload**: `{ "company_id": "uuid", "message_id": "uuid", "reason": "..." }`

### `whatsapp.send_requested` v1 / `whatsapp.sent` v1
- Similar shape.

---

## 10. Inbound

### `reply.received` v1
- **Emitted by**: webhook handler.
- **Payload**:
  ```json
  {
    "conversation_id": "uuid",
    "message_id": "uuid",
    "channel": "email | whatsapp",
    "from": "...",
    "body_preview": "..."
  }
  ```

### `meeting.booked` v1
- **Payload**: `{ "company_id": "uuid", "meeting_at": "...", "source": "manual | cal.com" }`

---

## 11. Outcome

### `deal.outcome_recorded` v1
- **Payload**:
  ```json
  {
    "company_id": "uuid",
    "deal_id": "uuid",
    "outcome": "won | lost | stalled",
    "amount_usd": 4500,
    "won_reason": "...",
    "lost_reason": "...",
    "objections_raised": [{ "text": "...", "response_used": "..." }]
  }
  ```
- **Consumed by**: embedding pipeline (upsert into Qdrant `past_wins` / `objections`).

---

## 12. Job-lifecycle (visibility only — not used for orchestration)

`job.created`, `job.leased`, `job.succeeded`, `job.failed`, `job.cancelled`, `job.reclaimed` — emitted by the worker for the dashboard's per-pipeline timeline view. Never consumed by sagas (sagas listen to domain events).

---

Links: [[rest]] · [[schemas]] · [[../02-architecture/event-driven]] · [[../07-database/schema]]
