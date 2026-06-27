# Architecture — Overview

> This is the canonical map of the system. Every other architecture file zooms into one piece of this.

---

## 1. The shape

Apollo is an **event-driven pipeline** of small specialist agents, fronted by an HTTP API and a Next.js dashboard, persisted to Postgres + Qdrant. It is designed to run on one box in V1 and to scale horizontally without rewrites.

```
                            ┌───────────────────┐
                            │   Next.js (UI)    │
                            │   App Router      │
                            └─────────┬─────────┘
                                      │ REST (OpenAPI)
                                      ▼
                            ┌───────────────────┐
                            │   apollo-api      │   Axum HTTP, thin handlers
                            │   (Rust)          │
                            └────┬────────┬─────┘
                                 │        │
                  enqueue jobs   │        │  read views
                                 ▼        ▼
                          ┌──────────────────────┐
                          │      Postgres        │  source of truth
                          │  - companies         │
                          │  - jobs (queue)      │
                          │  - events (log)      │
                          │  - artifacts         │
                          │  - llm_calls         │
                          │  - conversations     │
                          └─────┬────────┬───────┘
                                │        │
                  LISTEN/NOTIFY │        │  SQLx
                                ▼        ▼
                          ┌──────────────────────┐
                          │   apollo-worker      │   pulls jobs, runs agents
                          │   (Rust)             │
                          │                      │
                          │  ┌──────────────┐    │
                          │  │ Lead Disco   │    │
                          │  │ Crawler      │    │
                          │  │ Extractor    │    │
                          │  │ SEO Auditor  │    │
                          │  │ Tech Detect  │    │
                          │  │ BizIntel     │    │
                          │  │ Opportunity  │    │
                          │  │ ROI          │    │
                          │  │ Proposal     │    │
                          │  │ Email Writer │    │
                          │  │ WA Writer    │    │
                          │  │ Meet Coach   │    │
                          │  └──────┬───────┘    │
                          └─────────┼────────────┘
                                    │
                       ┌────────────┼────────────┐
                       ▼            ▼            ▼
                ┌──────────┐  ┌──────────┐  ┌──────────┐
                │  Gemini  │  │  Qdrant  │  │  Resend  │
                │  (LLM)   │  │ (vector) │  │  (mail)  │
                └──────────┘  └──────────┘  └──────────┘
```

---

## 2. Process model (V1)

Three Rust binaries:

| Binary | Job | Replicas (V1) | Scales by |
|---|---|---|---|
| `apollo-api` | HTTP for the dashboard + webhooks (Resend, future WA) | 1 | request volume |
| `apollo-worker` | Consumes jobs, runs agents | 1 (parallelism inside via Tokio tasks) | LLM concurrency |
| `apollo-cli` | Operator tooling: ingest URL, replay job, dump dossier | n/a | n/a |

All three share the same Postgres. The worker uses `LISTEN/NOTIFY` plus a periodic poll (5s) to claim jobs.

A scheduled task inside the worker (every 30s) reclaims jobs whose lease expired (worker died mid-job).

---

## 3. Events vs. jobs (terms)

These overlap conceptually; we keep them strictly separated.

- **Job** — a unit of work for a specific agent on a specific company. Has lifecycle (`queued → leased → running → succeeded | failed | cancelled`). Lives in the `jobs` table. Idempotency key: `(company_id, agent_name, input_hash)`.
- **Event** — an immutable fact about something that happened. Has type (`crawl.completed`, `email.sent`, etc.) and a payload. Lives in the `events` table. Published transactionally with the job-state transition that produced it.

Agents react to **events**, not to jobs directly. The event bus translates `event-type-of-interest` into job enqueues. This decoupling is why the architecture is called event-driven.

Full event catalogue in [[../06-api/events]]. Job lifecycle in [[queue-system]].

---

## 4. The pipeline as event chain

```
url.submitted
   → crawl.requested            ─► Crawler
   → crawl.completed            ─► Extractor
   → extraction.completed       ─► [SEO Auditor, Tech Detection, BizIntel] (parallel)
   → seo.completed              ┐
   → tech.completed             ├─ all three required ─► Opportunity Finder
   → bizintel.completed         ┘
   → opportunities.generated    ─► ROI Estimator
   → roi.completed              ─► [Email Writer, WhatsApp Writer, Proposal Writer] (parallel)
   → email.drafted              ┐
   → whatsapp.drafted           ├─ when all three ─► dossier.ready
   → proposal.drafted           ┘                       │
                                                        ▼
                                                  Operator review queue
                                                        │
                                              (review.approved)
                                                        ▼
                                                  email.send_requested
                                                        ▼
                                                  email.sent
                                                        ▼
                              (Resend inbound webhook → reply.received)
                                                        ▼
                                                  Conversation update
```

Fan-out, fan-in, and the "all required" semantics are handled by a small **saga coordinator** that lives in `apollo-events`. Sagas are defined declaratively (see [[event-driven]] §4).

---

## 5. Data flow per agent

Every agent looks like this:

```rust
#[async_trait]
pub trait Agent {
    type Input: DeserializeOwned + Hash;
    type Output: Serialize + JsonSchema;

    fn name(&self) -> &'static str;            // stable identifier
    fn version(&self) -> u32;                   // bump on behavior change
    fn input_schema() -> serde_json::Value;
    fn output_schema() -> serde_json::Value;

    async fn run(
        &self,
        ctx: &AgentContext,
        input: Self::Input,
    ) -> Result<Self::Output, AgentError>;
}
```

`AgentContext` carries: the `LlmClient` trait object, the `PromptLoader`, the `db` repository handle, a `correlation_id`, and a `cost_tracker`.

Agents do not know about the queue, the events bus, or HTTP. The worker wraps `run()` with:

1. Compute `input_hash`.
2. Look up cached output for `(name, version, input_hash)`. If hit, return it.
3. Open tracing span, attach `correlation_id`, `company_id`, `agent_name`.
4. Call `run()`.
5. Persist output as an `artifact`.
6. Publish completion event with `artifact_id`.

This is the **only** place that retry/cache/cost logic lives. Agents stay pure.

See [[agent-architecture]] for deeper internals.

---

## 6. LLM call discipline

Every LLM call goes through `LlmClient`:

```rust
#[async_trait]
pub trait LlmClient {
    async fn complete(
        &self,
        req: LlmRequest,    // model, messages, output_schema, max_output_tokens
    ) -> Result<LlmResponse, LlmError>;
}
```

The default `GeminiClient` implementation:

- Parses the response against `output_schema`. On mismatch, retries ONCE with a system message appended explaining the validation error. Second mismatch fails the call.
- Logs `(model, prompt_name, prompt_version, input_tokens, output_tokens, cost_usd, latency_ms, agent_name, company_id, correlation_id)` to `llm_calls` table.
- Honours per-call `timeout_ms` (default 30s).
- Refuses to send if `max_output_tokens` is unset.

This single chokepoint is why cost is measurable and why prompt versions are auditable.

---

## 7. Cost model

Apollo's unit cost target is **≤ $0.05 per researched company** (see [[../00-vision/success-metrics]]).

How we hit it:

- **Determinism before LLM** at every layer. The SEO Auditor uses Lighthouse + structured checks; the LLM only gets the *findings*, not the raw HTML. The Tech Detection uses Wappalyzer rules; the LLM is not involved at all. The Crawler turns HTML into Markdown deterministically before any agent sees it.
- **Model selection**: Gemini 2.5 Flash for everything except the Proposal Writer (Gemini 2.5 Pro) and the Meeting Coach (Pro). The cost difference dominates everything else.
- **Output token caps** per agent (set in the prompt frontmatter, enforced in `LlmClient`). Most agents are capped at 1500 output tokens; Proposal Writer at 3000.
- **Caching** keyed by `(prompt_name, prompt_version, input_hash)`. Re-runs are free.
- **Batching** where it doesn't degrade quality. The ROI Estimator processes all opportunities for a company in a single call, not one per opportunity.

A `cost_per_company` view in Postgres aggregates `llm_calls` per `company_id` and surfaces it on the dashboard. PRs that move the median above $0.05 are blocked.

---

## 8. Failure modes and how we handle them

| Failure | Detection | Handling |
|---|---|---|
| Crawler can't reach site | Reqwest error | Job fails with `category: "crawl.unreachable"`. Operator sees in dashboard. No retries beyond 2. |
| Site blocks bots (403, robots) | HTTP status + robots.txt | Stop. Mark `crawl.blocked`. Try headless Chromium on next replay if operator requests. |
| JS-heavy site, empty text | Extractor heuristic (< 200 words) | Fallback to headless Chromium for the same URL. If still empty, fail with `extraction.empty`. |
| LLM rate limit | 429 from Gemini | Exponential backoff up to 3 tries; surface as job retry. |
| LLM schema mismatch | JSON Schema validation | One re-prompt with the error; second failure marks the job failed. |
| Worker crash mid-job | Lease expires (5 min) | Lease reaper reclaims job; another worker picks it up. Idempotency key prevents double work. |
| Resend bounce | Inbound webhook | Mark contact `email.bounced`; pause sequence. |
| Postgres connection exhaustion | sqlx error | Worker reduces parallelism; alert. V1 cap is 20 connections; raise carefully. |

Every failure mode is testable; goldens for the worst-cases live under `apollo/09-experiments/`.

---

## 9. Multi-tenancy posture (forward-compatible)

V1 has a single operator, but every primary table has a `tenant_id` column (default `00000000-...-0001`). Repositories take a `Tenant` parameter. APIs scope by tenant. No public UI exposes it.

This costs us nothing in V1 and saves us a multi-week migration when V2 arrives.

---

## 10. Pointers

- Queue + lease semantics: [[queue-system]]
- Event bus, sagas, schemas: [[event-driven]]
- DB tables: [[database]]
- Agent internals: [[agent-architecture]]
- HTTP API: [[../06-api/rest]]
- Per-agent specs: [[../03-agents/_overview]]
