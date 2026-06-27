# Architecture Decision Records

> Every meaningful choice has an ADR. Append-only. Status updates are new entries that reference the old one.

Format: title · status · context · decision · consequences.

---

## ADR-001 · Documentation-first development

**Status**: Accepted

**Context**: Apollo is built by one operator with Claude Code as the implementation partner. LLM coding agents perform best when given dense, structured context. We do not want to lose institutional memory between sessions.

**Decision**: Every meaningful behavior and decision is documented in this Obsidian vault *before* code is written. `CLAUDE.md` at the root is the engineering constitution that Claude Code reads on every session.

**Consequences**:
- Higher upfront cost; lower long-term drift.
- Doc changes must be in the same PR as code changes that affect them.
- Documentation drift is the only bug we treat as P0.

---

## ADR-002 · Stack: Rust backend, Next.js frontend, Postgres, Qdrant

**Status**: Accepted

**Context**: Backend choice was operator preference (Rust). Frontend needed: dashboards, tables, forms, audit PDFs, generated quickly by Claude Code, with a mature component ecosystem.

**Decision**:
- **Backend**: Rust + Axum + Tokio + SQLx + Postgres. Reasons: operator preference, great long-running worker story, strong typing pairs well with structured-output LLM calls, single deployable per process.
- **Frontend**: Next.js 15 (App Router) + TypeScript strict + Tailwind + shadcn/ui + TanStack Query. Reasons: best Claude Code generation quality observed, richest component ecosystem, easy Vercel/Render deploys, App Router fits the read-heavy review queue UI.
- **DB**: PostgreSQL. Sole source of truth.
- **Vector**: Qdrant. Operator has prior experience; great Rust client.
- **LLM**: Google Gemini (Flash + Pro). Cost/quality balance at high volume.
- **Email**: Resend. Cheapest with usable webhooks.

Considered and rejected:
- **SvelteKit** frontend — leaner but smaller ecosystem and worse AI tooling support.
- **Leptos** Rust full-stack — keeps it Rust everywhere but the AI coding agents produce worse code; not a productivity win.
- **Supabase** as all-in-one — bundles auth/DB/storage but loses Rust's compile-time SQL checking and adds vendor risk.
- **Anthropic Claude API** as primary LLM — superior reasoning but ~3-5× cost; reconsider for Proposal Writer if Gemini Pro underperforms on goldens.

**Consequences**:
- Two deployables to operate (Rust API/worker + Next.js).
- Strong types across the stack via OpenAPI-generated TS clients.
- LLM provider change requires re-running goldens but is a one-file swap thanks to the `LlmClient` trait.

---

## ADR-003 · Event-driven via Postgres LISTEN/NOTIFY in V1

**Status**: Accepted

**Context**: We need a queue and an event bus. Operating Redis/NATS/Kafka in V1 is overhead for a single-operator system.

**Decision**: Use Postgres for both. SKIP LOCKED for the queue (`jobs` table); LISTEN/NOTIFY for the event bus, with a 5-second poll fallback for missed notifications.

**Consequences**:
- One less component to operate. Same transaction can write state + queue + event.
- Throughput ceiling around 50 events/s sustained; far above V1 demand (1–5/s expected).
- Migration to Redis Streams or NATS is planned for V2 (ADR-005, draft).

---

## ADR-004 · Documentation is the source of truth for prompts

**Status**: Accepted

**Context**: We need to iterate on prompts without redeploying. Operator must own them.

**Decision**: Prompts are Markdown files with frontmatter under `apollo/04-prompts/`. The Rust `PromptLoader` watches the directory and hot-reloads. Frontmatter declares version; loader refuses to serve body changes without version bumps.

**Consequences**:
- Operator changes prompts without touching `.rs` files.
- Prompt versions are recorded in `prompt_versions` and stamped on every output, making historical replays reproducible.
- A bad prompt change blocks reload until version is bumped; this is intentional friction.

---

## ADR-005 · Migration from Postgres bus → Redis Streams (planned, V2)

**Status**: Proposed (not yet decided to act)

**Context**: When sustained event rate exceeds ~30/s for more than a few minutes, Postgres NOTIFY starts to feel the pressure (NOTIFY does not back-pressure well; polling fallback dominates). At that point we want a real stream.

**Decision (proposed)**: Add a thin `EventBus` trait in `apollo-events`. V1 implementation: `PgListenBus`. V2 implementation: `RedisStreamsBus`. Cutover by config flag with both buses running for one week of dual-emit so we can A/B verify ordering and at-least-once delivery.

**Consequences**:
- Code change is one trait + two implementations + a small migration runner.
- Need to operate Redis (or use managed e.g. Upstash) from V2 onwards.
- Decide based on metric: `events_per_second_p95 > 20` for 5 consecutive days.

---

## ADR-006 · No autonomous outbound in V1

**Status**: Accepted

**Context**: Apollo writes outreach. Sending without human review risks brand damage, spam-list inclusion, and worse: unhappy prospects on a small UAE market where word travels.

**Decision**: All outbound messages (email, WhatsApp) require explicit operator approval. The dashboard's Approve button is the only path to send. No auto-send config flag exists.

**Consequences**:
- Operator is the throughput bottleneck. We accept that for trust.
- Re-evaluate post V3 once Apollo has a documented track record of clean drafts (target: >85% approved without edits over 100 sends).

---

## ADR-007 · Multi-tenant ready, single-tenant in V1

**Status**: Accepted

**Context**: V2 wants to open Apollo to other operators. V1 is one user. Bolting on multi-tenancy later is expensive.

**Decision**: Every primary table has `tenant_id UUID NOT NULL`. Repositories take a `Tenant` parameter. Default tenant is seeded in migration 001. No public UI exposes multi-tenancy in V1.

**Consequences**:
- Costs ~5% extra schema verbosity now.
- Saves a 2–3 week migration in V2.

---

## ADR-008 · Operator is also the only sales engineer

**Status**: Accepted (product/business, not technical)

**Context**: There is no team. Apollo's design must not assume one.

**Decision**: All flows are single-operator. No collaboration features (no comments, no assignment, no approvals matrix). Adding multi-user collaboration is a V3 question; until then, build for one.

**Consequences**:
- Faster shipping. Less code.
- V3 collaboration work is real; account for it when scoping V3.

---

## ADR-009 · Feature-Driven DDD layout

**Status**: Accepted

**Context**: The initial draft used a layer-based crate split (`apollo-agents`, `apollo-db`, `apollo-llm`). Touching one feature meant editing five crates; agents had no natural home; testing the workflow required spinning up infrastructure. Layer-based grew expensive fast.

**Decision**: Organize by **feature**, not by layer. Each capability is its own crate (`feature-research`, `feature-drafting`, …). Inside each crate, four boxes: `domain/`, `application/`, `infra/`, `presentation/`, plus a `configure.rs`. Cross-cutting capabilities live in `platform-*` crates that any feature may use.

**The one rule**: inner code never knows about outer code.
```
presentation → application → domain ← infrastructure
```
A CI lint enforces it by scanning every `feature-*/src/domain/` for forbidden imports.

**Cross-feature communication**: forbidden directly. Features talk via events on the bus or through shared types in `platform-core`. The bootstrap (each app's `main.rs`) is the only matchmaker.

**Consequences**:
- Adding a feature = create a new folder with the same shape; no surgery on existing folders.
- `domain` tests run in milliseconds.
- Swapping Postgres → another DB only touches `infra/` for affected features.
- Onboarding (human or Claude Code): read one feature, you've read them all.
- Slightly more Cargo.toml ceremony per feature; we accept it.

Full rules in [[02-architecture/feature-layout]]. Supersedes the layer-based layout in the initial draft of [[CLAUDE]] §2.

---

## ADR-010 · Multi-provider LLM routing (Gemini + Claude)

**Status**: Accepted

**Context**: Apollo's cost target is ≤ $0.05 per researched company. Structured-output agents (BizIntel, Opportunity, ROI) need cheap and reliable JSON. Human-facing copy agents (Email, WhatsApp, Proposal) need top-tier English copywriting and tone. No single provider is best at both at the right price.

**Decision**: Route per-agent via the `model:` frontmatter in each prompt file. Initial assignment:

| Agent kind | Model |
|---|---|
| Structured-output (BizIntel, Opportunity, ROI) | Gemini 2.5 Flash |
| Cold copy (Email Writer, WhatsApp Writer) | Claude Haiku 4.5 |
| Heavy reasoning (Proposal Writer, Meeting Coach) | Claude Sonnet 4.6 |

`LlmClient` trait already exists; `platform-llm` will ship Gemini and Anthropic adapters from Sprint 1. Per-prompt model selection means a switch is a 1-line frontmatter edit + golden re-run.

**Rejected**:
- DeepSeek V3.2 / GLM-4.6 as primary — cheaper but Chinese-hosted; awkward compliance story for UAE prospect data. Reconsider in V3 for non-customer-data agents only (Tech Detection, internal benchmarks).
- Anthropic for everything — output quality wins, costs ~3× more on the bulk-volume agents where it doesn't matter.
- Gemini for everything — Flash falters on cold-email tone; Pro is OK but uneven on persona-specific copy.

**Consequences**:
- Cost at projected V1 volume: ~$4–8/month total LLM spend at 200 companies/month. Well under the $0.05/company target with margin.
- Two SDKs to maintain in `platform-llm`. Trivial: both are HTTP.
- A failover policy is required: if Claude is down, Email Writer falls back to Gemini Pro with a flagged event (`email.drafted.model_fallback`). Defined in `platform-llm` config.

---

## ADR-011 · Supabase Postgres + Qdrant Cloud (free tier)

**Status**: Accepted

**Context**: Operator's choice of stack-as-a-service for V1: Supabase managed Postgres for SQL, Qdrant Cloud free tier for vectors. No self-hosted Docker for the dev database — fewer moving parts on the laptop, faster cold start.

**Decision**:
- **Postgres**: Supabase managed instance (free tier OK for V1; upgrade to Pro $25/mo when point-in-time-restore and longer WAL retention matter — Sprint 6 latest).
- **Vector store**: Qdrant Cloud free tier (1 GB RAM / 4 GB disk / single node — fits V1/V2 volumes with headroom). Two collections initially: `past_wins`, `objections`. `proposals` added when needed.
- **Connection**: `DATABASE_URL` points at Supabase pooled connection (port 6543, transaction mode) for the API and worker. Migrations apply via direct connection (port 5432). `QDRANT_URL` + `QDRANT_API_KEY` env vars.
- **SQLx**: compile-time query checking still works — Supabase is plain Postgres. `sqlx prepare` runs against the direct connection at build time.

**Notes vs. ADR-002**: ADR-002 said Postgres + Qdrant. This ADR locks in **which** Postgres and **which** Qdrant. The architecture is unchanged; only the hosting is decided.

**Consequences**:
- Zero local DB infra. `docker-compose.yml` becomes optional — useful for offline development but not required for the happy path.
- Free tier limits to watch:
  - Supabase free: 500 MB DB, 2 GB egress/mo, pauses after 7 days inactivity. Apollo's data fits comfortably; the "pause" behavior means we should ping the API every few days during slow weeks (or upgrade to Pro before launch).
  - Qdrant free: 1 GB RAM. ~1M vectors @ 768 dims with HNSW comfortably. Headroom for years.
- One extra env var to manage (`QDRANT_*`). Standard.
- Backup strategy: rely on Supabase's daily snapshots in V1; revisit when the operator's data hits commercial value (post-V3).
- When the worker runs heavy crawls, watch Supabase's pooler limits (60 concurrent on free). V1 worker caps concurrency at 16 — well under.

---

## How to add an ADR

1. Add a new entry at the bottom of this file with the next ADR number.
2. State the status as **Proposed** until accepted.
3. Reference any superseded ADR (e.g., "Supersedes ADR-005").
4. Commit alongside the code change that motivated it.

Links: [[PROJECT_BIBLE]] · [[../CLAUDE]]
