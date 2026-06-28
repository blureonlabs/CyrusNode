# Session summary — 2026-06-28

> Snapshot for the next session. Where we are, what shipped, what's pending,
> what to do first when you come back.

---

## State of the project

- **Branch**: `dev` at `bce520d` (pushed to `origin/dev`).
- **Tests**: 68 passing, `make ci` green.
- **Cost so far today**: $0.12 across 46 LLM calls (21 bizintel + 25 email-writer).
- **Operator-side**: 19 emails ready to send (10 limos + 9 movers), screenshots ready.

## What shipped (Sprint 1 + Sprint 2 + quality wave)

Whole platform is in a runnable state — `apollo discover → batch → status → queue → cost → clean` works end-to-end. Full commit log on `dev`:

```
bce520d  movers playbook + humanised v3 email writer + pitches doc
c6b0b3b  quality + hygiene wave: extraction cleanup, ops commands, BizIntel hints
ac87d52  S2-fix: site_facts → queue, cost ledger, parallel batch, queue UX
5d03b01  Sprint 2: discovery, send, queue, BizIntel field re-prompt
5d23dbf  feat(drafting): wire industry playbooks into email writer tone
7561497  E2E: live ingest works against real Dubai SMB
84c401a  T09–T13: end-to-end research → email pipeline
6128dda  security: SSRF guard on all crawler fetches
edc39f3  S1-T08: polite HTTP crawler
5a152b3  S1-T03..T07: worker loop, event bus, agent wrapper, prompt loader, LLM clients
01862e9  S1-T02: Postgres + migrations + repos via Supabase
512fed3  S1-T01: workspace scaffold (Feature-Driven DDD)
```

## Capabilities matrix

| Capability | Status |
|---|---|
| Find leads in a niche (Google Places) | ✅ `apollo discover` |
| Research each company (crawl + extract + audit) | ✅ |
| Personalized email per company (industry playbook tone) | ✅ |
| Humanized voice (v3 prompt — contractions, no marketing words) | ✅ |
| Auto-detect recipient (Cloudflare-decoded, validated) | ✅ |
| Industry playbook hints → BizIntel context | ✅ |
| Operator review queue with inline recipient prompt | ✅ |
| Send via Resend (with dry-run `.eml` fallback) | ✅ |
| Parallel batch (concurrency-bounded + 429 backoff) | ✅ |
| Cost tracking (per agent, per company, jsonl ledger) | ✅ |
| Operator hygiene (`apollo clean`, `apollo status`) | ✅ |
| SSRF guard on every crawler fetch | ✅ |
| Industry playbooks loaded (3) | ✅ dental_clinic, limousine_uae, movers_uae |
| Demo HTML dashboards for prospect screenshots | ✅ movers + limos |
| Reply tracking / conversation threading | ❌ Sprint 3 (spec'd) |
| Suggested reply drafts | ❌ Sprint 3 (spec'd) |
| Proposal Writer | ❌ Sprint 3 (spec'd) |
| Meeting brief from past wins | ❌ Sprint 5 |
| Persistent DB (Supabase wired but not used) | ❌ — runs on filesystem |

## Operator artifacts ready to use

- `out/dossiers/*.json` — 9 movers researched (also 10 limos earlier; cleaned).
- `out/outbox/*.txt` — 9 movers: TO + SUBJECT + BODY + WhatsApp mobile + WhatsApp message + intel.
- `out/demos/movers-ops-dashboard.html` — single-file demo, edit `{ Your Company }` placeholder then screenshot.
- `out/demos/limos-ops-dashboard.html` — same shape, indigo accent, fleet table.
- `apollo/10-playbooks/runbook.md` — operator workflow.
- `apollo/10-playbooks/pitches.md` — 6 framings (internal, external, one-liner, elevator, subject lines, AI-question framing).

## What's actually been sent

- **Movers batch**: 9 dossiers generated, files in `out/outbox/`. Operator has begun sending manually via own domain + own WhatsApp.
- **One known reply**: prospect said *"we have automation already"*. Suggested reply drafted (ask for the after-hours conversion %).

## Open architectural decisions

- **ADR-011** locked Postgres on Supabase, vectors on Qdrant Cloud. Neither connected yet — V1 runs filesystem.
- **ADR-012** locked Gemini-only routing (Flash for everything, Pro deferred).
- **DDD layout** holds. Domain-import lint enforces.
- **Single crate** (apollo) not workspace — operator's call from earlier.

## What to do FIRST when you come back

In priority order:

1. **Send the remaining outbox files.** Don't build before validating. The single most important signal is whether real Dubai SMBs reply.
2. **Track replies in a sheet.** Columns: `Sent date | Opened? | Replied? | Wanted call? | Notes`. The data is the asset.
3. **Reply to the "we have automation already" prospect** with the qualifying question from the previous session ("what % of after-hours WhatsApp inquiries convert to surveys within 24h?").
4. **Don't run more niches yet.** 19 sends across 2 niches is enough data for the first round.

## What NOT to do when you come back (avoid scope creep)

- Don't build Sprint 3 features (reply listener, proposal writer) before knowing if anyone replies.
- Don't add a third niche (cleaners, aquaguard) before processing the first 19 responses.
- Don't migrate from filesystem to Postgres yet.
- Don't build the Next.js dashboard.

## Sprint 3 candidates (when validated)

If/when 1+ booked calls land:
- **S3-T01** Resend webhook receiver (`out/replies/`)
- **S3-T02** Conversation tracking (`out/conversations/`)
- **S3-T03** Suggested-reply drafter (`apollo reply <conversation_id>`)
- **S3-T04** Proposal Writer agent
- **S3-T05** Replay any stage of the pipeline
- **S3-T06** Per-stage cost view
- Full spec in `apollo/08-development/sprint-3.md`

## Sprint 4+ candidates (much later)

- `apollo whatsapp` proper subcommand (right now WhatsApp drafts are templated, not LLM-generated)
- LinkedIn outbound channel
- Qdrant integration (past-wins retrieval for Meeting Coach)
- Next.js dashboard (still deferred to Sprint 6)
- `apollo demo --industry X --company Y` that auto-fills the dashboard placeholder

## Operator-side TODO

- [ ] Confirm employment contract permits freelance consulting (legal note in PROJECT_BIBLE §9)
- [ ] Pace the sends: 2/day from your domain, don't blast 19 at once
- [ ] Buy `GOOGLE_MAPS_API_KEY` so next batch is discover-driven (not WebSearch)
- [ ] Set `RESEND_API_KEY` if you want `apollo queue` to actually send (vs dry-run `.eml`)
- [ ] Add a "free pilot" candidate from your network for the first case study

## Quick resume commands

```bash
cd /Users/hariprasad/Documents/Projects/CyrusNnode
git pull origin dev
make ci                                                    # confirm green
./target/release/apollo-cli status                         # see current queue
./target/release/apollo-cli cost --window week             # see this week's spend
./target/release/apollo-cli industries                     # see loaded playbooks
cat out/outbox/01-*.txt                                    # check next to send
```

Links: [[../10-playbooks/runbook]] · [[../10-playbooks/pitches]] · [[../PROJECT_BIBLE]]
