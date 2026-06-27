# Apollo — Product Requirements Document

> Version: 0.1 (pre-V1). Owner: operator. Last reviewed: at kickoff.

This document is the contract between the product vision in [[../PROJECT_BIBLE]] and the engineering work in [[../08-development/sprint-1]]. Every shippable behavior lives here. If a behavior is not described in this PRD, it does not exist in V1.

---

## 1. Scope

### In scope (V1)

- Single-operator CLI + minimal Next.js dashboard.
- Ingest a single company URL or a CSV of URLs.
- Produce: business dossier, technical audit, opportunity report, email draft.
- Operator review queue with approve/edit/reject.
- Email send via Resend.
- Persistence of every artifact, every LLM call, every cost.
- Replayable pipeline (idempotent by content hash).

### In scope (V2)

- WhatsApp draft generation and outbound via 360dialog or Twilio.
- PDF proposal generator.
- Reply listener (inbound webhook → conversation thread).
- Lead discovery (Google Maps + LinkedIn search).
- Multi-channel sequencer.

### In scope (V3)

- Meeting coach with retrieval over past wins.
- Embeddings-backed similarity search (Qdrant).
- Multi-tenant flag (no UI for it; DB-scoped).

### Out of scope (V1, V2, V3)

- Public signup, billing, payments.
- Mobile app.
- Arabic-language outreach.
- Phone-call AI (Twilio Voice / Vapi). Maybe later.
- Generic chatbot or RAG-as-a-service.

---

## 2. Personas

See [[personas]] for the full descriptions. Briefly:

- **The Operator** (V1 user): One AI engineer running consulting. Highly technical. Time-poor. Cost-conscious. Wants every cycle spent on humans, not data.
- **The Prospect** (recipient of outreach): SMB owner or marketing lead at a Dubai business. 5–200 employees. Has WhatsApp and Instagram presence. Sees five generic AI-agency pitches a week and ignores them.
- **Future Operator** (V2 user): Other one-person AI consultants. Same shape as primary, different industries.

---

## 3. The end-to-end user journey (operator)

```
1. Operator drops a URL (or CSV) into the dashboard.
2. Apollo enqueues a Research Job.
3. Pipeline runs (crawl → extract → audit → research → opportunities → draft).
4. Notification arrives: "Dossier ready for ABC Dental Clinic."
5. Operator opens the dossier. Sees:
   - 1-paragraph business summary.
   - 5–10 audit findings, prioritized.
   - 3–5 AI opportunities with ROI estimates.
   - Pre-drafted email + WhatsApp + proposal outline.
6. Operator clicks Edit. Tweaks 1–2 sentences. Approves.
7. Send goes out via Resend. Status flips to Sent.
8. Reply arrives. Apollo threads it into the conversation view.
9. Operator drafts a reply with Apollo's help. Approves. Sends.
10. Meeting booked (Cal.com / manual). Apollo generates pre-call brief.
11. After the call, operator logs outcome + objection. Apollo stores it.
```

Full sequence diagram in [[user-journey]].

---

## 4. Feature inventory

Each feature has an ID. The sprint plans reference these IDs.

### F-001 — URL Ingest
- **Description**: Submit one URL or a CSV; system normalizes (strip UTM, lowercase host) and dedupes against existing companies.
- **Acceptance**: Submitting the same URL twice in one session does not create two `companies` rows. Submitting a CSV of 100 URLs returns 200 OK and enqueues 100 jobs in < 2s.
- **Sprint**: 1

### F-002 — Crawl + Extract
- **Description**: For a company, fetch homepage + sitemap-indexed pages up to a budget (default 25 pages, 2 MB total HTML). Convert to clean Markdown.
- **Acceptance**: On a healthy site, returns Markdown for every fetched page within 60s. On JS-heavy sites, falls back to headless Chromium. Records `robots_txt_blocked` if applicable and stops.
- **Sprint**: 1

### F-003 — SEO + Tech Audit
- **Description**: Lighthouse-style performance + accessibility + SEO scores via local Chromium runner. Wappalyzer-style stack detection.
- **Acceptance**: Returns numeric scores + a structured list of findings (e.g., `{ severity: "high", category: "mobile", description: "Mobile viewport not set", evidence: "..." }`).
- **Sprint**: 1

### F-004 — Business Intelligence
- **Description**: From the extracted Markdown + sitemap, an LLM agent produces a 5-field business summary: what they sell, who they serve, key value props, evident customer segments, and an estimated maturity tier (early / growing / established).
- **Acceptance**: Output validates against `business_summary.schema.json`. 95% of test fixtures produce non-null `what_they_sell`.
- **Sprint**: 1

### F-005 — Opportunity Finder
- **Description**: Given the business summary + audit findings + tech stack, the Opportunity Agent proposes 3–7 ranked AI services from the [[../05-knowledge/ai-services]] catalogue.
- **Acceptance**: Each opportunity has `name`, `evidence`, `effort_tier`, `roi_estimate`, `confidence`. Evidence MUST cite at least one specific page or finding.
- **Sprint**: 2

### F-006 — ROI Estimator
- **Description**: For each opportunity, conservative dollar estimates: monthly value, setup cost, ongoing cost. Uses industry baselines from the playbook.
- **Acceptance**: Numbers are bounded (no $1B claims). All currencies in USD; convert at the edge.
- **Sprint**: 2

### F-007 — Email Writer
- **Description**: Personalized cold email under 120 words. Subject line + body. References at least one specific opportunity and one specific evidence point. No greeting clichés.
- **Acceptance**: Goldens pass on test fixtures. Operator review queue shows generated email side-by-side with evidence used.
- **Sprint**: 1 (basic) → 2 (refined)

### F-008 — WhatsApp Writer
- **Description**: Personalized opener under 40 words. Mentions one specific business detail. No links in first message.
- **Acceptance**: Output length validated. Goldens pass.
- **Sprint**: 2

### F-009 — Proposal Generator
- **Description**: One-page PDF, branded. Sections: Problem · Proposed solution · Estimated outcome · Timeline · Next step.
- **Acceptance**: Renders to PDF in < 5s. Looks like a consulting deliverable, not a chatbot output.
- **Sprint**: 3

### F-010 — Review Queue
- **Description**: Operator UI listing dossiers ready for approval. Edit-in-place for email/WhatsApp/proposal. Approve & Send action.
- **Acceptance**: From inbox to approved-and-sent in ≤ 2 minutes for a familiar industry.
- **Sprint**: 2 (basic UI) → 3 (refined)

### F-011 — Email Send (Resend)
- **Description**: Outbound via Resend API. From-address per campaign. Threading via `Message-ID` and `In-Reply-To`.
- **Acceptance**: Bounces and complaints flip the contact status. Open and click tracking recorded.
- **Sprint**: 2

### F-012 — WhatsApp Send (manual + API)
- **Description**: Two modes. Manual mode: copy generated message to clipboard, operator pastes into WhatsApp Web. API mode (V3): send via 360dialog using a warmed business number.
- **Acceptance**: Manual mode logs that send happened via operator confirmation. API mode logs message ID + delivery status.
- **Sprint**: 2 (manual) → 4 (API)

### F-013 — Reply Listener
- **Description**: Inbound webhook from Resend → match by `In-Reply-To` → append to conversation.
- **Acceptance**: Reply appears in conversation view within 10s. If matching fails, lands in an "Unmatched" inbox.
- **Sprint**: 3

### F-014 — Conversation Thread
- **Description**: Per-company conversation view: every outbound, every reply, every status change. Generates Apollo-suggested next message.
- **Acceptance**: Operator can advance the conversation without leaving the page.
- **Sprint**: 3

### F-015 — Lead Discovery
- **Description**: Operator provides niche + city. Apollo scrapes Google Maps + (later) LinkedIn search for matching businesses, enqueues research jobs.
- **Acceptance**: For "dental clinics in Dubai," returns ≥ 50 leads with website + phone in ≤ 60s.
- **Sprint**: 4

### F-016 — Multi-channel Sequencer
- **Description**: Cadence per company: Day 1 email · Day 2 LinkedIn · Day 4 WhatsApp · Day 7 call reminder · Day 10 follow-up email · Day 15 final.
- **Acceptance**: Sequencer respects per-channel send windows (UAE business hours). Operator can pause or abandon a sequence at any step.
- **Sprint**: 4

### F-017 — Meeting Coach
- **Description**: Pre-call brief: business summary + key opportunities + relevant past wins (retrieved via embeddings) + likely objections.
- **Acceptance**: Generated in < 10s on demand. Operator can mark items as "used" for retrospective analysis.
- **Sprint**: 5

### F-018 — Cost & Observability Dashboard
- **Description**: Live view of cost per company today, this week, this month. Per-agent latency p50/p95. Number of LLM calls retried due to schema mismatch.
- **Acceptance**: Renders within 1s for the last 30 days of data.
- **Sprint**: 6

### F-019 — Replay
- **Description**: Operator can replay any agent step on any company. Existing cached results are invalidated for that step only; downstream steps re-run on the new output.
- **Acceptance**: Replay button on every step of a dossier. Replays are tracked with reason.
- **Sprint**: 3

### F-020 — Playbook Loader
- **Description**: Industry playbooks (Markdown under `apollo/05-knowledge/industries/`) loaded at runtime. Each playbook defines: typical pains, typical opportunities, conversion benchmarks, sample email tone.
- **Acceptance**: Adding a new `industries/<name>.md` requires zero code changes; first researched company in that industry picks it up.
- **Sprint**: 2

---

## 5. Non-functional requirements

- **Privacy**: Scraped company content is stored; never republished. Email content stored encrypted at rest in V2+.
- **Reliability**: Pipeline must survive a worker crash mid-job. Jobs are reclaimed after 5 minutes of no heartbeat.
- **Throughput**: 50 companies in parallel without saturating Postgres connections (use a connection pool, cap at 20 connections in V1).
- **Latency**: See [[../00-vision/success-metrics]].
- **Observability**: Every event published to the event log table with `correlation_id`. Tracing spans cover every agent, every LLM call, every external HTTP call.
- **Auditability**: Every outbound message is reproducible from stored inputs + the prompt version used.

---

## 6. Open questions

- Do we want a "shared dossier" link the operator can send to a prospect as a teaser? (Lean: yes, V3.)
- Do we want Cal.com integration in V1 or use a static booking URL? (Lean: static URL V1; integration V3.)
- Should the proposal generator output a Notion page instead of a PDF? (Lean: PDF first; Notion later if it shortens deal cycles.)
- How do we handle non-English sites? (Lean: detect language; if not English, skip with a flagged reason in V1.)

Track these in [[../DECISIONS]] as they resolve.

---

Links: [[roadmap]] · [[features]] · [[personas]] · [[user-journey]] · [[../PROJECT_BIBLE]]
