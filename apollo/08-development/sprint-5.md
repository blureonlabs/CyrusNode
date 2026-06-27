# Sprint 5 — Meeting coach + long-term memory

## Scope
- F-017 Meeting Coach
- Qdrant integration: `past_wins`, `objections`, `proposals` collections
- Embedding pipeline triggered by `deal.outcome_recorded`

## Stories
- S5-T01 — `apollo-embedder` crate: trait + Vertex AI embeddings impl.
- S5-T02 — Qdrant collections + indexing; idempotent upsert.
- S5-T03 — Outcome-recording flow: operator logs `outcome` + objections after a meeting; event triggers upserts.
- S5-T04 — Meeting Coach agent + prompt + schema + 5 staged-meeting goldens.
- S5-T05 — Meetings API + UI: register meeting, view brief, log outcome.

## Definition of done
- Operator can run a real meeting using the brief without taking other notes.
- ≥ 4/5 talking points rated "I would have said this anyway."
- Outcome logging round-trips: a recorded win shows up in the next similar company's brief.

Links: [[sprint-4]] · [[sprint-6]]
