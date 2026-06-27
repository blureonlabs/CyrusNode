# Sprint 6 — Polish + observability

## Scope
- F-018 Cost & Observability Dashboard
- Ops dashboard (pipeline health, recent failures, dead-letter queue)
- Production deploy story
- Public-facing demo (sample dossier the operator can show on a call)

## Stories
- S6-T01 — `cost_per_company_daily` materialized view + refresh schedule.
- S6-T02 — Dashboard: cost (today/week/month), per-agent latency p50/p95, retry-due-to-schema-mismatch rate.
- S6-T03 — Dead-letter view: failed jobs with last error; one-click re-queue.
- S6-T04 — Production deploy: Render (apollo-api + apollo-worker) + managed Postgres + Qdrant Cloud.
- S6-T05 — Demo URL: a sample anonymized dossier the operator can show prospects to demonstrate the platform.
- S6-T06 — Backup verification: restore a `pg_dump` into a scratch instance and run smoke tests against it.

## Definition of done
- Operator can demo Apollo end-to-end on a sales call without touching the CLI.
- Cost dashboard reflects the last 30 days within 5 minutes of activity.
- Restored backup passes smoke test.
- Apollo is production-deployable. Operator now spends time on humans, not code.

Links: [[sprint-5]] · [[../01-product/roadmap]]
