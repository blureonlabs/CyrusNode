# Clients

Long-term memory. One Markdown file per company that Apollo has meaningfully touched.

File name: `<host>.md` (e.g. `abcdental-ae.md`). Subfolders by region: `dubai/`, `uae/`, `other/`.

## Template

```markdown
---
company_id: <uuid>
host: abcdental.ae
industry: dental_clinic
country: AE
city: Dubai
outcome: pending | won | lost | stalled
---

# ABC Dental Clinic

## Summary
1-line description.

## Problems found
Bulleted, from the audit.

## Opportunities pitched
Linked to [[../03-agents/opportunity-finder]] outputs.

## Emails sent
- 2026-06-27 — opener, subject "..."
- 2026-07-01 — follow-up

## Meeting notes
Date, attendees, key questions, decisions.

## Proposal
Link to the PDF artifact + a summary.

## Outcome
Won/lost reason. Objection raised. What we learned.
```

These notes are read by humans (the operator), but their structured content is also picked up by the embedder pipeline for Qdrant retrieval (Sprint 5).

Links: [[../03-agents/meeting-coach]] · [[../02-architecture/database]]
