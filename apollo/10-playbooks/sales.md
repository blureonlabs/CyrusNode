# Sales Playbook

> How the operator runs the human side of Apollo. Apollo handles research and drafting; this file handles the rest.

## The funnel
1. **Research** — Apollo produces a dossier.
2. **Outreach** — operator reviews and approves the email + WhatsApp.
3. **Reply handling** — operator converts replies to booked calls.
4. **Discovery call** — operator runs a 30-minute call from Apollo's brief.
5. **Proposal** — Apollo's one-pager + a short Loom recording of the dossier highlights.
6. **Close** — light back-and-forth, signed engagement letter, deposit invoice.
7. **Deliver** — actual build (outside Apollo's scope).

## Targets
- 200 researched companies / month (Apollo).
- 100 of those reach outreach (operator filters in review).
- 5–10 reply (5–10%).
- 3–5 booked calls.
- 1–2 paid engagements.

## Rules of engagement
- **Never pitch without research.** If Apollo's dossier doesn't surface something specific, the operator does not send.
- **Always pitch one outcome.** Multiple opportunities go in the proposal; the cold message is one outcome only.
- **Free pilot only if strategic.** Use free pilots to seed lighthouse case studies in the chosen first niche. Cap at 2–3 total free pilots before pricing.
- **Quote in AED for UAE clients.** Convert at the edge; internal numbers stay USD.

## Pricing posture
- Discovery audit: $500–$1500 fixed.
- Build engagements: $5k–$25k.
- Retainer: $1.5k–$5k/month.
- Never quote off the cuff. Generate a one-pager (Apollo) → send.

## Outreach cadence (the human channel mix)
| Day | Channel | What |
|---|---|---|
| 1 | Email | Apollo-drafted, operator-approved cold message. |
| 2 | LinkedIn | Connection request, no pitch. |
| 4 | WhatsApp | Apollo-drafted opener. |
| 7 | Email | Soft bump ("worth a quick look?"). |
| 10 | LinkedIn | One-line note referencing the email. |
| 15 | Email | Final breakup ("closing the loop"). |

## Reply triage rubric
- **Positive — meeting interest**: book within 24 hours. Use Apollo's `meeting.brief_ready` workflow.
- **Positive — info request**: send proposal PDF + 90-second Loom of the dossier.
- **Neutral — "send more info"**: send proposal but downgrade priority; don't chase.
- **Soft no — wrong time**: log objection in `deal.outcome_recorded`, schedule check-in in 90 days.
- **Hard no — wrong fit**: log objection ("we don't take new vendors"); close politely; remove from sequences.

## What NOT to do
- Don't expose Apollo. The operator is the brand; Apollo is internal tooling.
- Don't claim AI-engineer experience unless the prospect asks. The pitch is outcomes, not credentials.
- Don't oversell free pilots. They cost real time. Two or three lighthouse clients are enough.
- Don't run a sequence without reviewing every step. Apollo drafts; the operator approves.

Links: [[discovery]] · [[pricing]] · [[objection-handling]] · [[../PROJECT_BIBLE]]
