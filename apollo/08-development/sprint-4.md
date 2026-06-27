# Sprint 4 — Discovery + sequencing

## Scope
- F-015 Lead Discovery (Google Maps Places)
- F-016 Multi-channel Sequencer (Day 1 email → Day 4 WhatsApp → Day 10 follow-up)
- F-012 WhatsApp Send via 360dialog API

## Stories
- S4-T01 — Discovery agent: Google Maps Places + dedupe by host. Enqueues `company.create_requested` per candidate.
- S4-T02 — Discovery UI: form (niche + city + limit) + run progress + candidate review (operator can prune before research kicks off).
- S4-T03 — Sequencer model + worker: `sequences` + `sequence_steps` tables; scheduler walks ready steps every minute; respects UAE business-hours window per channel.
- S4-T04 — 360dialog integration: send WhatsApp template + freeform messages; webhook for delivery + replies.
- S4-T05 — Sequencer UI: per-company sequence view; pause/resume/abandon.

## Definition of done
- Operator inputs "dental clinics in Dubai" → 50 leads in the queue within 60s.
- One sequence runs through all 4 steps for a test company across a 15-day simulation window.

Links: [[sprint-3]] · [[sprint-5]]
