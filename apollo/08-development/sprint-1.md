# Sprint 1 — Foundation pipeline (CLI only)

> Two-week sprint. End state: one URL → email draft on disk, end-to-end, in under 2 minutes, for under 5 cents. No frontend yet.

## Goal

Establish the spine of Apollo. Get one company URL through the deterministic and first-LLM agents, produce an email draft, and prove the cost target.

## Scope (features from [[../01-product/PRD]])

- F-001 URL Ingest (CLI form)
- F-002 Crawl + Extract
- F-003 SEO + Tech Audit (basic — Lighthouse subset)
- F-004 Business Intelligence
- F-007 Email Writer (basic version, no industry tone yet)

Out of scope: opportunity finder, ROI, WhatsApp, proposal, dashboard, sequencer.

---

## Story-by-story (with acceptance criteria)

### S1-T01 — Workspace scaffold (Feature-Driven DDD)
- Create the Rust workspace per [[../02-architecture/feature-layout]].
- Platform crates (empty libs): `platform-core`, `platform-db`, `platform-queue`, `platform-events`, `platform-llm`, `platform-prompts`, `platform-crawler`, `platform-observability`. (`platform-embedder` deferred to Sprint 5.)
- Feature crates needed in Sprint 1: `feature-research`, `feature-drafting`. Each scaffolded with the four boxes (`domain/`, `application/`, `infrastructure/`, `presentation/`) + a `configure.rs`. Empty modules are fine.
- Binaries: `apollo-api`, `apollo-worker`, `apollo-cli`. Each gets a minimal bootstrap that calls `feature_*::configure(deps)` and merges routes/subscriptions.
- Add a `xtask` (or shell script in `Makefile`) that enforces the dependency rule: for every `crates/feature-*/src/domain/`, fail the build if any file imports `reqwest`, `sqlx`, `axum`, `tokio::net`, or any other `feature-*`/non-`platform-core` crate.
- Set up `cargo fmt`, `cargo clippy -D warnings`, `cargo test`, and the domain-import lint in `Makefile`. Basic GitHub Actions workflow runs `make ci`.
- **Done when**: `make ci` passes; planting `use sqlx::PgPool;` inside `feature-research/src/domain/dossier.rs` makes the build fail with a clear error.

### S1-T02 — Postgres + migrations + sqlx-cli
- `docker-compose.yml` with Postgres 16 + Qdrant (Qdrant not yet used; up for V5).
- Migrations 001–006: tenants, companies, contacts, jobs, events, artifacts, llm_calls.
- `apollo-db` repositories for: `CompaniesRepo`, `JobsRepo`, `EventsRepo`, `ArtifactsRepo`, `LlmCallsRepo`.
- `sqlx::query!` macros compile against running DB via `SQLX_OFFLINE=true` + `sqlx prepare`.
- **Done when**: `cargo sqlx prepare --workspace` succeeds and `cargo test -p apollo-db` passes against a testcontainers Postgres.

### S1-T03 — Job queue with SKIP LOCKED
- Implement `JobsRepo::claim_next` per [[../02-architecture/queue-system]] §3.
- Implement the reaper task in `apollo-worker`.
- Implement `enqueue<A: Agent>` helper in `apollo-core`.
- **Done when**: load test — 1000 jobs enqueued, 8 workers, 0 duplicates processed, 0 leaks.

### S1-T04 — Event bus on LISTEN/NOTIFY
- Add `events` insert with `NOTIFY events` in same transaction.
- Worker subscribes; falls back to polling every 5s.
- Saga skeleton (`apollo-events`) with one saga: `analysis` (still stubbed; just logs receipts).
- **Done when**: an integration test publishes 3 events and the saga emits its derived event within 2s.

### S1-T05 — Agent trait + worker wrapper
- Implement `Agent` trait in `apollo-core` per [[../02-architecture/agent-architecture]].
- Implement worker wrapper (cache lookup → run → persist artifact → publish event).
- Implement `AgentRegistry`.
- **Done when**: a stub agent that returns `Hello {url}` is registered, claimed, run, cached, and observably emits a completion event.

### S1-T06 — Prompt loader
- Read all `apollo/04-prompts/*.md`. Parse frontmatter via `serde_yaml`.
- Validate against required keys.
- Watch directory for changes via `notify` crate. On change, refuse to serve a body whose SHA-256 differs from the recorded version's hash (refuses to silently overwrite a version).
- Insert into `prompt_versions` on first observed (name, version).
- **Done when**: editing a prompt body without bumping version raises an error in the watcher logs; bumping the version reloads successfully.

### S1-T07 — Gemini client
- Implement `LlmClient` trait + `GeminiClient` impl in `apollo-llm` using `reqwest` and the official Gemini REST API.
- Validate output JSON against the prompt's output schema; one retry on mismatch.
- Persist `llm_calls` row on every call (success or failure).
- Cap `max_output_tokens` enforcement.
- **Done when**: a unit test against a mocked endpoint records the row; a manual test against the real endpoint produces a valid `business-summary` output.

### S1-T08 — `feature-research`: crawler adapter + port
- Define `CrawlPort` in `feature-research/src/domain/ports.rs`.
- Implement `HttpCrawlerAdapter` in `feature-research/src/infrastructure/`, built on `platform-crawler`.
- Per-host concurrency=1, 500ms delay, polite UA, robots.txt respect, sitemap parsing, BFS within budget.
- Save HTML to local disk (object storage in V6).
- **Done when**: crawling 5 well-known UAE SMB sites returns ≥ 10 pages each within 60s and respects `robots.txt`. `cargo test -p feature-research domain::` runs in < 200ms and touches zero I/O.

### S1-T09 — `feature-research`: extractor adapter + port
- Define `ExtractPort` in domain.
- Implement `MarkdownExtractorAdapter` in infrastructure (readability pass + html2md + heuristic extractors: phones via libphonenumber, emails, WhatsApp links, social).
- Empty-extraction guard publishes `extraction.empty` via `EventPublisher` port.
- **Done when**: on 5 fixtures, output total words ≥ 500, site_facts.phones non-empty for ≥ 4.

### S1-T10 — `feature-research`: SEO auditor adapter (basic)
- Define `SeoPort` in domain.
- Implement `LighthouseSeoAdapter` in infrastructure: spawn Chromium, capture 4 scores + 5 deterministic findings (viewport, favicon, og:image, robots, sitemap).
- Defer fancier findings to Sprint 2.
- **Done when**: 5 fixtures produce scores and findings without crashing.

### S1-T11 — `feature-research`: business-intelligence agent + use case
- Define `BizIntelPort` in domain and `BusinessSummary` value object.
- Implement `BizIntelAgent` in infrastructure, wiring `business-summary` prompt v1 via `platform-llm` + `platform-prompts`.
- Implement `application/research_company.rs` use case that orchestrates crawl → extract → audit + bizintel (parallel) and assembles a `DossierBase`.
- Compile schemas via `schemars`; write to `apollo/04-prompts/schemas/business-summary.v1.json`.
- **Done when**: 10 fixtures run through; ≥ 9 produce ≥ 3 evidence items and non-null `what_they_sell`. The use case test substitutes fake ports for every adapter.

### S1-T12 — `feature-drafting`: email-writer agent + use case
- Define `DraftPort` (or specifically `EmailDraftPort`) in `feature-drafting/src/domain/ports.rs`.
- Implement `EmailWriterAgent` in infrastructure, wiring `email-writer` prompt v1 with default tone (no industry-specific yet).
- Implement `application/draft_email.rs` use case that calls the port and enforces banned-phrase post-check by re-prompting on violation.
- `feature-drafting` subscribes to `bizintel.completed` events via `presentation/subscriptions.rs` and triggers the use case.
- **Done when**: 10 fixtures produce emails ≤ 120 words, with ≥ 2 anchors, and 0 banned phrases. End-to-end: `apollo ingest <URL>` produces `email.drafted` event.

### S1-T13 — CLI: `apollo ingest <URL>`
- One subcommand that:
  1. Inserts the company.
  2. Submits `url.submitted` event.
  3. Waits (polling) for `email.drafted`.
  4. Prints the email to stdout + writes to `./out/<host>.email.md`.
- **Done when**: `apollo ingest https://example.ae` runs end-to-end in ≤ 2 minutes on a clean Postgres.

### S1-T14 — Cost report
- `apollo report cost --since <date>` aggregates `llm_calls` by company and prints a table.
- **Done when**: After 5 ingests, the report shows total cost ≤ $0.25 (target $0.05 × 5).

### S1-T15 — Telemetry baseline
- `tracing` + `tracing-subscriber` with env filter.
- Console output (compact) in dev, JSON in prod.
- Every span includes `correlation_id`, `company_id`, `agent_name`.
- **Done when**: a single ingest's spans visibly chain by correlation_id in console output.

---

## Definition of done (the sprint as a whole)

- `apollo ingest <URL>` produces a usable email draft for any of 10 chosen UAE SMB URLs.
- Median cost per company ≤ $0.05.
- Median latency ≤ 90s.
- All goldens for `business-summary` and `email-writer` are checked in.
- `make ci` green: fmt, clippy, tests.
- README updated with current state.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Gemini API quirks → schema-mismatch retries blow cost | Cap max_output_tokens hard; alert if `schema_retry = true` rate > 5% |
| Lighthouse runner flaky → SEO Auditor blocks pipeline | SEO Auditor's failure should not block BizIntel + Email (saga only requires bizintel for V1's email path) |
| Crawler too slow on big sites | Cap pages and bytes; surface `crawl.budget_exceeded` cleanly |
| sqlx-prepare offline cache rot | Add `sqlx prepare` to pre-commit hook |

## Pointers for the next sprint
After Sprint 1 ships, [[sprint-2]] adds opportunity finder, ROI, WhatsApp draft, and the minimal Next.js dashboard.

Links: [[../01-product/PRD]] · [[../02-architecture/overview]] · [[../03-agents/_overview]] · [[../04-prompts/_overview]]
