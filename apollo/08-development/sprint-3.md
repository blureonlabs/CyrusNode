# Sprint 3 — Conversations + proposals

## Scope
- F-009 Proposal Generator (PDF)
- F-013 Reply Listener (Resend inbound webhook)
- F-014 Conversation Thread view
- F-019 Replay (per-agent replay button)

## Stories (high level)
- S3-T01 — Proposal Writer agent + prompt + schema + 10 goldens. Uses Gemini Pro.
- S3-T02 — PDF renderer (Tera/Handlebars template → headless Chromium print). Branded template under `apollo/05-knowledge/proposal-template/`.
- S3-T03 — Resend webhook handler: verifies signature, matches `In-Reply-To` → conversation, emits `reply.received`.
- S3-T04 — Conversations API + UI: thread view, suggested reply (LLM), send reply.
- S3-T05 — Replay endpoint + UI: per-agent replay with reason + optional hint passed to BizIntel.
- S3-T06 — Drafting saga update: include proposal in the bundle. `dossier.ready` requires all three drafts.

## Definition of done
- 5 proposals generated and judged "looks like a consulting deliverable" by operator.
- 3 real replies threaded successfully.
- Replay verified end-to-end on a stuck pipeline.

Links: [[sprint-2]] · [[sprint-4]]
