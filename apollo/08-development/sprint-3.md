# Sprint 3 — Conversations + send-side telemetry + proposals

> Updated post-Sprint-2 ship. Drops the "build a UI" framing from the original plan since the CLI is the V1 surface. Focuses on the **loop close**: knowing what happened after `apollo queue` approved a send.

## Scope

In priority order:

- **S3-T01 — Resend webhook receiver** (`F-013`). Verify HMAC, persist event rows, log to `out/replies/<ts>-<event>.json`. Operator visibility into delivered/bounced/opened/clicked/replied without a UI.
- **S3-T02 — Conversation tracking** (`F-014`). Each sent QueueItem moves from `out/sent/` to `out/conversations/<company_id>/` and accumulates inbound replies. `apollo conversations` lists them.
- **S3-T03 — Suggested reply drafter**. New CLI: `apollo reply <conversation_id>` reads the thread + dossier, calls Gemini to draft a contextual response, opens it in `$EDITOR`, approves to send.
- **S3-T04 — Proposal Writer** (`F-009`). Same shape as bizintel/email-writer. Output: structured JSON for the one-page proposal. Sprint 4 wires the PDF renderer.
- **S3-T05 — Replay** (`F-019`). `apollo replay <company_id> --from <stage>` re-runs from any pipeline stage. Reads the cached artifacts to avoid redoing work.
- **S3-T06 — Per-channel cost view**. `apollo cost --group-by stage` breaks LLM spend down by pipeline stage (research/draft/proposal/reply).

## Stories — detail

### S3-T01 — Resend webhook receiver

- Endpoint: `POST /webhooks/resend`. Live on `apollo-api`.
- HMAC verify against `RESEND_WEBHOOK_SECRET` (Svix-compatible).
- Event types we care about: `email.sent`, `email.delivered`, `email.bounced`, `email.complained`, `email.opened`, `email.clicked`. (Replies arrive via inbound forwarding, not this hook — see S3-T02.)
- Persist as `out/replies/<ts>-<event>-<msg_id>.json`. Keep raw payload + extracted summary line.
- `apollo events --since today` summarizes recent events to stdout.

### S3-T02 — Conversation tracking

- Inbound reply path: Resend "inbound email" feature OR forwarding the operator's actual inbox (Gmail "send mail as"). For V1 we assume the operator forwards replies to a sentinel address `replies+<message_id>@<our-domain>` and the webhook parses the routing.
- On `email.sent`: copy `out/sent/<id>.json` → `out/conversations/<id>/dossier.json`, create `out/conversations/<id>/messages.jsonl`. Append the outbound as the first message.
- On `email.delivered`/`bounced`/`opened`: append a status row to messages.jsonl.
- On reply: append an inbound message row.
- `apollo conversations` lists every conversation with last-event date + status.

### S3-T03 — Suggested reply drafter

- New prompt `apollo/04-prompts/reply-writer.md`.
- Context: dossier (BusinessSummary + site_facts) + the conversation thread so far.
- Output JSON: `{ subject (if reply needs new), body, recommended_action: send|wait|do_not_reply, reasoning }`.
- CLI: `apollo reply <conversation_id>` prints the suggestion, drops body to `$EDITOR`, then prompts approve/send/discard.

### S3-T04 — Proposal Writer agent

- Mirror BizIntelAgent shape. Loads `proposal-writer` prompt; calls Gemini Pro (only place Pro stays after ADR-012).
- Output: structured JSON matching the proposal schema already in `apollo/04-prompts/proposal-writer.md`.
- Persist to `out/dossiers/<id>.proposal.json` alongside the QueueItem. The PDF renderer (S4) reads it.

### S3-T05 — Replay

- `apollo replay <company_id> --from crawl|extract|seo|bizintel|email`.
- Read the cached artifact (whatever's at `out/crawls/<id>/` etc.) and re-run forward from that stage.
- The artifact cache layer in `platform::queue` keyed by `(agent, version, input_hash)` already supports this — just wire the CLI.
- Bonus: `--hint "they're a manufacturer, not retail"` appended to the BizIntel prompt for one-shot correction.

### S3-T06 — Per-stage cost

- Tag each LLM call with `stage` (research/draft/proposal/reply) in `LlmRequest`. The `agent_name` field already exists; just add `stage` as a derived view in `apollo cost`.

## Definition of done

- 1 real reply received and threaded into a conversation file.
- 1 suggested reply drafted by `apollo reply` and approved by operator.
- 3 proposals generated; structured JSON validates against schema.
- 1 replay run end-to-end on a known-stuck dossier.
- `apollo cost --group-by stage` shows non-zero rows for at least 3 stages.

## Explicitly NOT in Sprint 3

- Next.js dashboard. CLI is the V1 surface; UI deferred to Sprint 6.
- Multi-tenant scoping. The schema supports it; the CLI doesn't expose it.
- Meeting coach (S5).
- Qdrant integration (S5).

## Risks

- Resend inbound email config is fiddly; the forward-to-sentinel approach may need DNS work. If it stalls, fall back to: `apollo reply` reads an `.eml` file the operator pastes from their inbox.
- Replay correctness depends on artifact cache hashing. Verify on Day 1 with a known-stuck URL before building more on it.

Links: [[sprint-2]] · [[sprint-4]]
