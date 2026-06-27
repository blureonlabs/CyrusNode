# User Journey — Operator

A day in Apollo, end to end.

## Morning routine (10 minutes)

1. Operator opens dashboard. Sees:
   - 12 new dossiers ready for review (overnight pipeline).
   - 3 replies from yesterday's outreach.
   - 1 meeting today, with brief pre-generated.
2. Operator skims the 3 replies first. Each shows the original outbound, the reply, and a suggested next message.
3. Two are positive ("interested, can you tell me more"). One is a soft no.
4. Operator edits the suggested replies, hits Approve. Apollo sends.

## Reviewing dossiers (30 minutes)

1. Operator opens the queue. Each dossier card shows:
   - Company name + 1-line description.
   - Top 3 audit findings.
   - Top 3 opportunities + ROI estimates.
   - Generated email + WhatsApp draft.
2. For each dossier (~2 minutes):
   - Skim the business summary. Does it ring true?
   - Skim the opportunities. Are any wrong?
   - Edit the email if needed.
   - Approve → goes to Send queue.
3. If a dossier is wrong (e.g., the business is actually a manufacturer, not a retailer), operator clicks "Replay" with a corrected hint. Apollo re-runs Business Intelligence and downstream agents.

## Outbound (5 minutes)

1. Send queue holds approved drafts. Operator confirms send. Email goes via Resend.
2. WhatsApp messages: operator copies one at a time, pastes into WhatsApp Web, confirms in Apollo that the send happened (V1). In V3, Apollo sends via 360dialog API.

## Discovery (variable)

1. Operator wants 50 more dental clinics. Types `dental clinics Dubai` into the discovery form.
2. Apollo scrapes Google Maps, dedupes against existing companies, enqueues 50 new research jobs.
3. Operator's evening pipeline produces 50 dossiers overnight.

## Meeting prep (5 minutes before call)

1. Operator opens the meeting brief. Reads:
   - Business summary refresher.
   - 3 key opportunities the operator pitched in the email.
   - 2 similar past wins (retrieved from Obsidian / Qdrant) with anonymized one-liners.
   - 3 likely objections + suggested responses.
2. Operator joins the call.

## Post-call (5 minutes)

1. Operator logs the outcome (interested / passed / next-step set / closed).
2. Logs any objection raised.
3. If next-step, Apollo schedules a follow-up reminder.

## Why this matters

Each step above is a feature in [[PRD]]. The journey defines the dashboard's information architecture and the prompts' priorities. If a feature doesn't appear in this journey, it doesn't ship in V1.

Links: [[PRD]] · [[personas]] · [[../PROJECT_BIBLE]]
