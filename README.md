# Apollo

> AI Consulting Platform. Research. Analyze. Recommend. Convert.

Apollo turns a single company URL into a complete sales workflow: business research, technical audit, AI opportunity report, personalized outreach (email + WhatsApp), proposal, and CRM tracking — fully automated, niche-agnostic, designed to be run by one operator.

This repository is **documentation-first**. Code is generated from the documentation, not the other way around. The Obsidian vault under `apollo/` is the product brain. The `CLAUDE.md` file at the root is the engineering constitution that Claude Code reads before every change.

---

## Quick start (for a human reader)

1. Open this folder in Obsidian as a vault (`File → Open Vault → /Users/hariprasad/Documents/CyrusNnode`).
2. Start at `apollo/PROJECT_BIBLE.md`. Follow the wiki-links.
3. Once aligned on scope, open this folder in Claude Code. Claude reads `CLAUDE.md` automatically.
4. Drive development sprint by sprint via `apollo/08-development/sprint-1.md`.

## Quick start (for Claude Code)

Before any code change, you MUST:

1. Read `CLAUDE.md` in full.
2. Read the relevant sprint file (`apollo/08-development/sprint-N.md`).
3. Read the agent / module spec under `apollo/03-agents/` or `apollo/02-architecture/`.
4. Confirm the acceptance criteria with the operator before implementing.

If the documentation is ambiguous, **update the documentation first**, then write code. Documentation drift is the only bug we don't tolerate.

---

## Repository layout

```
.
├── README.md                  # This file
├── CLAUDE.md                  # Engineering constitution (read by Claude Code)
└── apollo/                    # Obsidian vault — product brain
    ├── PROJECT_BIBLE.md       # Master narrative; entry point
    ├── DECISIONS.md           # Architecture Decision Records (ADRs)
    ├── TODO.md                # Live work tracker
    ├── 00-vision/             # Why this exists
    ├── 01-product/            # PRD, roadmap, personas, features
    ├── 02-architecture/       # System design, events, DB, queues
    ├── 03-agents/             # Specs for every AI agent
    ├── 04-prompts/            # Production prompt templates (hot-reloadable)
    ├── 05-knowledge/          # AI services catalogue, industry playbooks
    ├── 06-api/                # REST + event contracts
    ├── 07-database/           # Schema, ER diagram, indexing
    ├── 08-development/        # Sprint plans
    ├── 09-experiments/        # Prompt tests, model benchmarks, cost runs
    ├── 10-playbooks/          # Sales, discovery, pricing, objections
    ├── 11-clients/            # Per-client notes (becomes long-term memory)
    ├── 12-meetings/           # Meeting notes
    └── 13-daily-notes/        # Engineering daily log
```

## Stack at a glance

| Layer | Choice | Why |
|---|---|---|
| Backend | Rust + Axum + Tokio | Operator's preference; great for long-running scrape/LLM workers |
| Database | PostgreSQL + SQLx | Compile-time-checked queries; LISTEN/NOTIFY for events V1 |
| Vector store | Qdrant | Operator has prior production experience |
| Queue | Postgres job table (SKIP LOCKED) V1 → Redis Streams V2 | Start simple, upgrade when load demands |
| LLM | Gemini 2.5 Flash (cheap) + Gemini 2.5 Pro (heavy) | Cost/quality balance for high-volume research |
| Frontend | Next.js 15 (App Router) + TypeScript + Tailwind + shadcn/ui | Best Claude Code generation quality, mature ecosystem |
| Observability | tracing + OpenTelemetry → Grafana/Tempo | Per-agent latency and token cost tracking is mandatory |

Full reasoning lives in `apollo/DECISIONS.md`.

---

## License

Private. Not for distribution.
