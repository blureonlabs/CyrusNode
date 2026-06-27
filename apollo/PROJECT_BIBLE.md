# Apollo — Project Bible

> The master narrative. If you read only one file, read this one.

---

## 1. What Apollo is

Apollo is an **AI Consulting Platform** that takes a single company URL and produces, end-to-end:

1. A business research dossier (what they sell, who their customers are, how they operate).
2. A technical audit (website performance, SEO, tech stack, automation gaps).
3. An AI opportunity report (which AI services would move the needle for *this* company specifically, with ROI estimates).
4. A personalized outreach package (email + WhatsApp + one-page PDF proposal).
5. A CRM record that tracks the conversation from first message to booked meeting to signed contract.

It is operated by **one person**. It is designed to compete against AI agencies that sell buzzwords by selling *specific researched outcomes*. Personalization is the moat. Cost discipline is the constraint.

---

## 2. The thesis

Most AI agency outreach fails because the sender hasn't done the research. Most personalized outreach fails because doing the research doesn't scale.

Apollo's bet: **the research itself can be a product**. Once we can produce a credible 5-page audit of any company in under 2 minutes for under 5 cents, the outreach writes itself, the proposal writes itself, and the discovery call becomes a confirmation of work already done.

The operator's job stops being "find leads and write emails" and becomes "review the dossier, sign off, and talk to humans."

---

## 3. Who it's for

- **Primary user (V1)**: The operator — an AI engineer running a one-person consulting practice targeting SMBs in Dubai and the broader UAE/GCC market.
- **V2 user**: Other AI consultants and small agencies (Apollo as SaaS).
- **V3 user**: In-house sales teams at AI-forward agencies who want to replace cold-email SDR tools with research-grade outreach.

V1 is built for the operator only. SaaS shape is preserved in architecture (multi-tenant DB, scoped APIs) but no auth UI, no billing, no public signup.

---

## 4. The pipeline (one diagram)

```
Company URL
     │
     ▼
[Lead Discovery]        ── finds companies if you give it a niche/region
     │
     ▼
[Crawler]               ── fetches HTML, screenshots, robots, sitemap
     │
     ▼
[Extractor]             ── HTML → clean Markdown, structured facts
     │
     ▼
[SEO Auditor]           ── Lighthouse-style checks, schema, mobile, speed
     │
     ▼
[Tech Detection]        ── Wappalyzer-style stack identification
     │
     ▼
[Social Intelligence]   ── Instagram/LinkedIn presence and posting cadence
     │
     ▼
[Business Intelligence] ── what they sell, customer segments, value props
     │
     ▼
[Opportunity Finder]    ── candidate AI services mapped to detected problems
     │
     ▼
[ROI Estimator]         ── conservative dollar estimates per opportunity
     │
     ▼
[Proposal Writer]       ── one-page PDF, branded
     │
     ▼
[Email Writer]          ── personalized cold email
     │
     ▼
[WhatsApp Writer]       ── short, specific opener
     │
     ▼
[Operator Review]       ── HUMAN GATE before any send
     │
     ▼
[Outreach Sender]       ── email via Resend, WhatsApp manual or via API
     │
     ▼
[Reply Listener]        ── inbound webhook, threads conversation
     │
     ▼
[Meeting Coach]         ── pre-call brief, talking points, objection prep
     │
     ▼
[CRM] ──────────────────── single source of truth across all steps
```

Every arrow is an **event**. No module calls the next directly. See [[02-architecture/event-driven]].

---

## 5. Engineering principles (short form)

1. **Documentation precedes code.** This vault is the source of truth.
2. **One concern per agent.** Specialists, not god-prompts.
3. **Determinism before LLM.** Rules and regexes first, models last.
4. **Structured I/O at every boundary.** JSON Schema in, JSON Schema out.
5. **Prompts are data.** Markdown files, hot-reloadable, versioned.
6. **Cost is a first-class metric.** Tracked per call, per agent, per company.
7. **Operator is always in the loop** for anything sent to a human.
8. **Idempotency by content hash.** Re-runs cost nothing.
9. **Long-term memory in Obsidian.** Every closed deal becomes a note.
10. **Build for one user, design for a hundred.**

The expanded versions of these live in `CLAUDE.md` at the repo root.

---

## 6. The product brain

Every interaction Apollo has with a company becomes a permanent linked note under `apollo/11-clients/`. The notes link to the prompts that generated outputs, the opportunities that were pitched, the meeting notes that came after, and the outcome.

Over time the vault becomes a graph: searching `dental` surfaces every dental client, every proposal sent, every objection raised, every win/loss reason. New companies in the same vertical benefit from this history automatically — the proposal writer retrieves similar past wins via embeddings before generating.

This is the platform's compounding advantage. Code is replicable; the curated knowledge is not.

---

## 7. Roadmap shape

- **Sprint 1** — Foundation: ingest URL → research → audit → personalized email. No frontend yet; CLI only. See [[08-development/sprint-1]].
- **Sprint 2** — Opportunity finder + ROI + WhatsApp output. Add minimal Next.js dashboard.
- **Sprint 3** — Proposal generator (PDF). Reply listener via Resend inbound webhooks. CRM table.
- **Sprint 4** — Lead discovery (Google Maps + scraping). Multi-channel sequencer with day-by-day cadence.
- **Sprint 5** — Meeting coach. Embeddings-based retrieval over past wins.
- **Sprint 6** — Polish, observability, ops dashboard, public-facing demo.

The full roadmap with acceptance criteria lives in [[01-product/roadmap]].

---

## 8. Non-goals (what Apollo deliberately does NOT do)

- **Not a chat product.** The operator is not chatting with Apollo. Apollo runs pipelines and presents reviewable outputs.
- **Not an autonomous SDR.** Sending is human-gated in V1, V2, V3. We can revisit later.
- **Not a generic CRM.** The CRM table exists to track Apollo-managed conversations. We don't import contacts from elsewhere.
- **Not multi-language at V1.** English outreach only. Arabic comes after V3.
- **Not a frontend-first product.** The dashboard is for review, not for content creation.

---

## 9. The legal note (carried over from kickoff)

The operator is currently employed as an AI engineer. Whether freelance consulting is permitted depends on the employment contract — specifically clauses on outside employment, moonlighting, non-compete, conflict of interest, IP ownership, and confidentiality. Apollo must **never** touch employer code, data, models, or time. The operator is responsible for confirming this with their employer or a lawyer before invoicing a client. Apollo does not address this concern technically; it's flagged here so the assumption is shared.

---

## 10. Where to go next

- New to the project? Read [[00-vision/vision]], [[00-vision/mission]], [[00-vision/success-metrics]].
- About to write code? Read `CLAUDE.md` at the repo root, then [[08-development/sprint-1]].
- Designing a new agent? Read [[03-agents/_overview]] and the pattern files there.
- Curious about cost? Read [[02-architecture/overview]] section 7.
- Want to understand the stack? Read [[DECISIONS]] — every choice has an ADR.
