# Sprint 2 — Opportunities + minimal dashboard

> Goal: turn the email draft into a real dossier with opportunities and ROI, and put it behind a queue the operator can clear.

## Scope
- F-005 Opportunity Finder
- F-006 ROI Estimator
- F-008 WhatsApp Writer
- F-010 Review Queue (basic — list + view + approve)
- F-011 Email Send (Resend)
- F-012 WhatsApp Send (manual mode only)
- F-020 Playbook Loader

## Stories (high level)
- S2-T01 — Industry playbook loader: load `apollo/05-knowledge/industries/*.md`, merge into prompt context.
- S2-T02 — Opportunity Finder agent + prompt + schema + 10 goldens.
- S2-T03 — ROI Estimator agent + prompt + schema + 10 goldens.
- S2-T04 — WhatsApp Writer agent + prompt + schema + 10 goldens.
- S2-T05 — Drafting saga (fan-in over email + whatsapp; proposal added in Sprint 3).
- S2-T06 — `apollo-api` skeleton: Axum + utoipa + auth middleware + `/companies`, `/dossiers`, `/dossiers/{id}/approve`.
- S2-T07 — Next.js app: dashboard with companies list, dossier view, approve/edit actions. shadcn/ui setup.
- S2-T08 — Resend integration: send email, store `external_id`.
- S2-T09 — WhatsApp manual mode: copy-to-clipboard with confirmation; logs `whatsapp.sent`.

## Definition of done
- Operator can ingest a URL, see the dossier in the dashboard within 2 minutes, edit, approve, send.
- 5 real emails sent to volunteer recipients; ≥ 1 reply.
- Median cost per dossier ≤ $0.05.

Links: [[sprint-1]] · [[sprint-3]]
