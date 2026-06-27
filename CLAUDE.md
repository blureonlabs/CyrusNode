# CLAUDE.md — Engineering Constitution for Apollo

You are working on **Apollo**, an AI Consulting Platform. This file is the constitution. It overrides defaults and habits. Read it before every coding session.

If anything below conflicts with a user instruction in the moment, ask the operator which wins. Do not silently override.

---

## 0. The one-paragraph context

Apollo ingests a company URL and produces, end-to-end, a research dossier, a technical + AI-opportunity audit, a personalized outreach package (email + WhatsApp + a one-page proposal), and a CRM record that tracks the conversation through to a booked meeting. The platform is **niche-agnostic** but configured per industry via Markdown playbooks. The operator is a single AI engineer running it solo. Quality of personalization is the moat. Throughput is the constraint. Cost per researched company is the metric we obsess over.

---

## 1. How to behave in this codebase

1. **Documentation is source of truth.** Before writing code, read the relevant doc under `apollo/`. If the doc is wrong or missing, update it FIRST in the same PR.
2. **One concern per module.** If a function does crawling AND extraction, split it. Each agent answers exactly one question (see `apollo/03-agents/`).
3. **Event-driven by default.** Modules do not call each other directly. They emit events; other modules subscribe. The event contracts live in `apollo/06-api/events.md`.
4. **Structured I/O everywhere.** Every agent has a JSON Schema for input and output. Validate on the way in and on the way out. Untyped LLM output is a bug.
5. **Determinism before LLM.** If a rule, regex, or HTML check can answer the question, use it. Only escalate to an LLM when the deterministic layer cannot decide. This is non-negotiable for cost.
6. **Hot-reloadable prompts.** Prompts live in `apollo/04-prompts/*.md` and are loaded at runtime. NEVER inline prompt strings in `.rs` source. The loader watches the directory and reloads on change.
7. **Cost is observable.** Every LLM call records `model`, `input_tokens`, `output_tokens`, `cost_usd`, `latency_ms`, `agent`, `company_id` to the `llm_calls` table. PRs that add a call without instrumentation are rejected.
8. **No silent fallbacks.** If the crawler fails, the event must say so. Do not return an empty result that downstream code mistakes for "no problems found."
9. **Idempotency.** Every job has an idempotency key (usually `(company_id, agent_name, input_hash)`). Re-running a job with the same key must return the cached result, not redo the work.
10. **Test the prompts.** Prompts have golden files in `apollo/09-experiments/prompt-tests/`. A prompt change requires re-running goldens and committing the diffs.

---

## 2. Stack and conventions

### Backend (Rust)
- **Framework**: `axum` 0.7+. Handlers are thin; logic lives in `services/`.
- **Async runtime**: `tokio` with multi-thread runtime.
- **DB**: `sqlx` with compile-time query checking against a local Postgres. Migrations via `sqlx migrate`. Never use string-formatted SQL — always parameterized.
- **HTTP client**: `reqwest` with a shared `Client` per service (connection pooling).
- **Serialization**: `serde` everywhere. No `serde_json::Value` in domain types — define structs.
- **Errors**: `thiserror` for library errors, `anyhow` for application-level glue. Public APIs return typed errors; never `anyhow::Error` across crate boundaries.
- **Logging/tracing**: `tracing` + `tracing-subscriber` with `EnvFilter`. Every span includes `company_id` and `correlation_id`.
- **Config**: `figment` reading `config/{default,local,prod}.toml` plus env overrides. No `dotenv` in production code.
- **Time**: `chrono` with `Utc` only in domain. Convert to local at the edges.

### Folder layout (single crate — Feature-Driven DDD)

We organize code **by feature, not by layer**. **One Cargo crate**, with `platform/` and `features/` as module trees inside `src/`. Three thin binaries via `src/bin/`. See [[apollo/02-architecture/feature-layout]] for the full rules.

```
/
├── Cargo.toml                       # one crate, name = "apollo"
├── src/
│   ├── lib.rs                       # crate entry
│   ├── bootstrap.rs                 # wires platform + features for any binary
│   │
│   ├── bin/                         # thin entry points — call into the lib only
│   │   ├── apollo-api.rs            # Axum HTTP server
│   │   ├── apollo-worker.rs         # queue consumer
│   │   └── apollo-cli.rs            # operator CLI
│   │
│   ├── platform/                    # shared kernel — capabilities, not product behavior
│   │   ├── core/                    # base types: Ids, Clock, Tenant
│   │   ├── db.rs                    # Postgres pool + migrations runner
│   │   ├── queue.rs                 # jobs table, SKIP LOCKED, Agent trait
│   │   ├── events.rs                # event bus + saga runtime
│   │   ├── llm.rs                   # LlmClient trait + Gemini + Claude impls
│   │   ├── prompts.rs               # hot-reloading prompt loader
│   │   ├── crawler.rs               # HTTP fetcher, headless Chromium, HTML→Markdown
│   │   └── observability.rs         # tracing + OpenTelemetry setup
│   │
│   └── features/                    # one module per bounded context
│       ├── research/                # crawl → audit → bizintel (the dossier base)
│       │   ├── mod.rs
│       │   ├── configure.rs         # the one wiring file
│       │   ├── domain/              # types + rules. ZERO I/O deps.
│       │   │   ├── mod.rs
│       │   │   ├── dossier.rs
│       │   │   └── ports.rs         # traits application asks of infra
│       │   ├── application/         # use cases (workflows)
│       │   │   ├── mod.rs
│       │   │   └── research_company.rs
│       │   ├── infra/               # adapters that implement the ports
│       │   │   ├── mod.rs
│       │   │   ├── crawler_adapter.rs
│       │   │   ├── extractor_adapter.rs
│       │   │   ├── seo_runner.rs
│       │   │   └── bizintel_agent.rs
│       │   └── presentation/        # HTTP routes + event subscriptions for this feature
│       │       ├── mod.rs
│       │       ├── routes.rs
│       │       └── subscriptions.rs
│       │
│       ├── drafting/                # email + whatsapp + proposal drafts (Sprint 1+)
│       ├── discovery/               # niche+city → company candidates (Sprint 4)
│       ├── synthesis/               # opportunities + ROI (Sprint 2)
│       ├── review/                  # operator queue, approve/reject, replay (Sprint 2)
│       ├── outreach/                # sender + sequencer + outbound webhooks (Sprint 2+)
│       ├── conversations/           # threads, inbound webhooks, suggested replies (Sprint 3)
│       ├── meetings/                # meeting brief + outcome logging (Sprint 5)
│       └── intelligence/            # Qdrant retrieval — past_wins, objections (Sprint 5)
│
├── migrations/                      # SQL migrations
├── scripts/
│   └── check-domain-imports.sh      # CI lint that enforces the dependency rule
└── apollo/                          # the Obsidian vault — docs are still source of truth
```

**Dependency direction (the one rule):**

```
presentation → application → domain ← infra
```

- `domain` knows nobody. Imports `serde`, `thiserror`, `chrono`, `uuid`, and `platform::core` — that's it.
- `application` imports `domain` (its own feature's) only.
- `infra` imports `domain` (to implement ports) and `platform::*` modules. It does NOT import `application` or `presentation`.
- `presentation` imports `application` and `domain`. Never `infra`.

If any file under `src/features/*/domain/` imports `sqlx`, `reqwest`, `axum`, `tokio::net`, `crate::platform::db`, `crate::platform::llm`, `crate::platform::crawler`, `crate::features::*::infra`, or any other I/O surface — **you've broken the rule**. `scripts/check-domain-imports.sh` (run as part of `make ci`) fails the build.

**Cross-feature communication is forbidden directly.** Feature A never `use crate::features::other::…`. Features talk via events on the bus, or via shared types exported from `platform::core`. `bootstrap.rs` is the only matchmaker — the single file that knows about all features.

**When to make a port (trait):**
- Make one when multiple infra implementations will exist (real vs. fake for tests, Postgres vs. another store, Gemini vs. Claude).
- Don't make one for boot-time-only things — a factory function is plenty.

### Frontend (Next.js)
- **Framework**: Next.js 15 App Router, TypeScript strict mode.
- **Styling**: Tailwind + shadcn/ui. No CSS-in-JS, no styled-components.
- **State**: Server Components by default. `use client` only when necessary. TanStack Query for client-side fetching.
- **Forms**: `react-hook-form` + `zod`.
- **API client**: Auto-generated from the Rust backend's OpenAPI spec via `openapi-typescript`. NEVER hand-write request types.
- **Folder layout**:
  ```
  app/
    (dashboard)/
      companies/
      campaigns/
      audits/
    (auth)/
  components/ui/      # shadcn primitives
  components/apollo/  # Apollo-specific components
  lib/api/            # Generated client + wrappers
  lib/utils/
  ```

---

## 3. Naming, style, and code review

- **Snake_case** for Rust everything except types (`PascalCase`) and constants (`SCREAMING_SNAKE`).
- **Domain language** matches the docs. If `apollo/03-agents/opportunity.md` calls it an `OpportunityCandidate`, the type is `OpportunityCandidate` — not `Opp`, not `Suggestion`.
- **No abbreviations** in public APIs. `company_id`, not `co_id`.
- **No `unwrap()`** in production code paths. `unwrap()` is fine in tests and in `main.rs` startup.
- **Functions over 60 lines** trigger a refactor question. Functions over 100 lines are a hard fail.
- **Public functions are documented** with `///` comments that state inputs, outputs, and failure modes. Private helpers do not need docs.
- **One commit, one concern.** A commit that touches the crawler and the email writer is two commits.

---

## 4. Decision-making protocol

When you face a meaningful choice (new dependency, new pattern, new module boundary):

1. Check `apollo/DECISIONS.md`. If a related ADR exists, follow it.
2. If no ADR covers it, **stop and ask the operator** before writing code. Propose 2–3 options with tradeoffs.
3. Once decided, write a new ADR and commit it in the same PR.

Examples of choices that require an ADR:
- Adding a new external service (any new SaaS, new database, new queue).
- Changing the event schema (events are a public contract).
- Introducing a new agent or removing one.
- Switching LLM provider or model family.
- Any change that alters cost-per-company by more than 20%.

---

## 5. Working with prompts

- Prompts are Markdown files with frontmatter:
  ```markdown
  ---
  name: opportunity-finder
  version: 3
  model: gemini-2.5-flash
  temperature: 0.2
  inputs: [business_summary, tech_stack, seo_report, problems]
  outputs: [opportunities[]]
  ---
  You are a senior AI consultant...
  ```
- The loader parses frontmatter, validates inputs, hot-reloads on file change.
- Output is parsed against a JSON Schema in `apollo/04-prompts/schemas/`. Schema mismatches retry once, then fail loudly.
- Prompt changes require a golden-file update under `apollo/09-experiments/prompt-tests/<prompt-name>/`. Each golden is a (input fixture, expected output) pair.
- Never edit a prompt without bumping `version:`. The DB records which prompt version produced which output for audit and regression analysis.

---

## 6. LLM cost discipline

Apollo will process hundreds of companies per month. Cost compounds.

Hard rules:

- **Default model is Gemini 2.5 Flash.** Use Pro only when Flash provably underperforms on the golden set.
- **Cap output tokens** on every call. Most agents need 800–2000 output tokens, not 8000.
- **Cache aggressively.** Crawl results, parsed Markdown, deterministic checks, and LLM outputs are all keyed by content hash. Re-runs hit the cache.
- **Batch where possible.** If you can process 5 companies in one prompt with consistent results, do that.
- **Track cost per company** as a first-class metric. Surface it in the dashboard. Alert if a single company's research costs > $0.50.
- **Never log full prompt text** to stdout in production. Costs CPU and disk, and prompts may contain client data. Log prompt name + version + input hash instead.

---

## 7. Testing

- **Unit tests** for all deterministic logic (extractors, scorers, parsers). Use `#[cfg(test)]` modules.
- **Integration tests** for repositories — spin up a real Postgres in a `testcontainers` container.
- **Agent tests** are golden tests. Real LLM calls are mocked behind the `LlmClient` trait. Use recorded responses checked into `apollo/09-experiments/`.
- **End-to-end smoke** runs the full pipeline on 3 known-good URLs and asserts on structured output fields, not full text.
- A PR is not mergeable if `cargo test --all` fails or if any golden has changed without a corresponding `apollo/09-experiments/` update.

---

## 8. What NOT to do

- Do not introduce a new LLM provider without an ADR.
- Do not add a "just for now" hardcoded prompt. There is no "for now."
- Do not store credentials in code or in `.env` files committed to the repo. Use 1Password/Doppler/Render env at runtime.
- Do not log PII (emails scraped from sites, phone numbers) at INFO level. Use DEBUG with explicit `client_data = true` field.
- Do not modify the event schema in a breaking way without writing an upcasting migration.
- Do not add a feature that wasn't on the sprint plan without confirming with the operator. Scope creep is the second-most-expensive bug after documentation drift.

---

## 9. The PR checklist (you read this before opening a PR)

- [ ] Documentation updated (`apollo/...`) before code.
- [ ] ADR added if architecture changed.
- [ ] All new public functions have `///` docs.
- [ ] No `unwrap()`, no `println!`, no `dbg!`.
- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all` all green.
- [ ] Goldens updated if a prompt was touched.
- [ ] OpenAPI spec regenerated if an HTTP route changed.
- [ ] Frontend types regenerated if the OpenAPI changed.
- [ ] Cost impact estimated for changes touching LLM calls. Note it in the PR description.
- [ ] One concern per commit, conventional commit message.

---

## 10. Pointers

- Start here: `apollo/PROJECT_BIBLE.md`
- What we're building this sprint: `apollo/08-development/sprint-1.md`
- Why the stack is what it is: `apollo/DECISIONS.md`
- The pipeline in one diagram: `apollo/02-architecture/overview.md`
- All AI agents: `apollo/03-agents/`
- All prompts: `apollo/04-prompts/`
