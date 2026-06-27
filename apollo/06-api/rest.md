# REST API

> Apollo's HTTP surface. Thin Axum handlers calling service-layer functions. OpenAPI auto-generated from `utoipa` annotations.

---

## 1. Conventions

- Base URL: `/api/v1`
- All requests authenticated (V1: shared bearer token in `Authorization: Bearer <token>`). V2: per-user JWT.
- All responses are JSON. Errors follow RFC 7807 problem+json:
  ```json
  { "type": "...", "title": "...", "status": 400, "detail": "...", "trace_id": "..." }
  ```
- Pagination: `?cursor=<opaque>&limit=<n>`. Responses include `next_cursor` (null at end).
- Idempotency: writes accept `Idempotency-Key` header (UUID); replay returns the original response for 24h.
- All times in ISO-8601 UTC.
- All IDs are UUIDs (except event IDs, which are ULIDs).

---

## 2. Endpoints (V1)

### Companies

| Method | Path | Purpose |
|---|---|---|
| POST | `/companies` | Submit one URL → enqueue pipeline. |
| POST | `/companies/bulk` | Submit a CSV or JSON array of URLs. |
| GET | `/companies` | List companies (filters: `status`, `industry`, `city`, `q`). |
| GET | `/companies/{id}` | Full company view (links to latest artifacts). |
| POST | `/companies/{id}/replay` | Replay one or more agents (body: `{"agents":["bizintel","email-writer"]}`). |

### Dossiers (the reviewable bundle)

| Method | Path | Purpose |
|---|---|---|
| GET | `/dossiers` | Queue of dossiers waiting for review. |
| GET | `/dossiers/{company_id}` | The reviewable dossier (summary + opportunities + drafts). |
| POST | `/dossiers/{company_id}/approve` | Approve drafts → schedule send. |
| POST | `/dossiers/{company_id}/reject` | Reject drafts with reason. |
| PATCH | `/dossiers/{company_id}/drafts/{kind}` | Edit a draft in place (`kind` ∈ `email \| whatsapp \| proposal`). |

### Outbound

| Method | Path | Purpose |
|---|---|---|
| POST | `/outbox/email/{message_id}/send` | Trigger immediate send (else scheduler picks up). |
| GET | `/outbox` | List queued + recently sent messages. |

### Conversations

| Method | Path | Purpose |
|---|---|---|
| GET | `/conversations` | List, filtered by status. |
| GET | `/conversations/{id}` | Full thread (messages + suggested next reply). |
| POST | `/conversations/{id}/draft-reply` | Generate a suggested reply (LLM). |
| POST | `/conversations/{id}/messages` | Send a manual reply. |

### Discovery (V4)

| Method | Path | Purpose |
|---|---|---|
| POST | `/discovery/runs` | Start a discovery run (niche + city + limit). |
| GET | `/discovery/runs/{id}` | Status + candidates. |

### Sequences (V4)

| Method | Path | Purpose |
|---|---|---|
| POST | `/sequences` | Start a multi-channel sequence on a company. |
| GET | `/sequences/{id}` | Sequence status + per-step state. |
| POST | `/sequences/{id}/pause` | Pause. |
| POST | `/sequences/{id}/resume` | Resume. |
| POST | `/sequences/{id}/abandon` | Terminate. |

### Meetings (V5)

| Method | Path | Purpose |
|---|---|---|
| POST | `/meetings` | Register a scheduled meeting; enqueues brief generation. |
| GET | `/meetings/{id}/brief` | Get the brief. |
| POST | `/meetings/{id}/outcome` | Log outcome (won/lost/next-step + objections). |

### Operations

| Method | Path | Purpose |
|---|---|---|
| GET | `/ops/health` | Liveness probe. |
| GET | `/ops/ready` | DB + LLM reachability. |
| GET | `/ops/cost/today` | Aggregate cost (today, week, month). |
| GET | `/ops/pipeline-health` | View backed by `pipeline_health` SQL view. |

### Webhooks

| Method | Path | Purpose |
|---|---|---|
| POST | `/webhooks/resend` | Resend inbound email + bounce notifications. |
| POST | `/webhooks/whatsapp` | (V4) Inbound WhatsApp via 360dialog. |

Webhook handlers verify HMAC signatures from the provider. Verification failure → 401 + alert.

---

## 3. OpenAPI generation

Annotations via `utoipa`:

```rust
#[utoipa::path(
    post,
    path = "/api/v1/companies",
    request_body = CreateCompany,
    responses(
        (status = 201, body = Company),
        (status = 409, body = Problem)
    ),
    tag = "companies"
)]
pub async fn create_company(...) -> Result<Json<Company>, ApiError> { ... }
```

`apollo-api` exposes the spec at `/openapi.json`. The frontend generates its TypeScript client via `openapi-typescript`. Drift between spec and frontend types is a CI failure (regenerate then commit).

---

## 4. Authentication (V1)

Single shared bearer token, stored in operator's password manager. Set as env var `APOLLO_API_TOKEN`. Middleware compares with `subtle::ConstantTimeEq`.

V2 adds JWT issued by a tiny auth route; multi-user; per-tenant scoping at the middleware.

---

## 5. Errors

Every handler returns `Result<T, ApiError>`. `ApiError` is typed:

```rust
pub enum ApiError {
    NotFound,
    Conflict(String),
    Validation(Vec<FieldError>),
    Forbidden,
    Internal(anyhow::Error),
}
```

`From<ApiError> for axum::response::Response` emits problem+json. `Internal` logs with full backtrace + correlation_id; client sees only `trace_id`.

---

Links: [[events]] · [[schemas]] · [[../02-architecture/overview]]
