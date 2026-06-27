# Meetings

One Markdown file per scheduled or completed meeting.

File name: `YYYY-MM-DD-<company-slug>.md`.

## Template
```markdown
---
company_id: <uuid>
meeting_at: 2026-07-02T14:00:00+04:00
attendees: [Hari, Ahmed]
status: scheduled | held | no-show | rescheduled
outcome: pending | next-step | proposal-sent | won | lost
---

# Meeting — ABC Dental

## Brief (auto-generated)
Paste the Meeting Coach output here, or link to the artifact.

## Live notes (during the call)
- ...

## After-call summary
- Key decisions.
- Specific commitments and dates.
- Objections raised + which response worked.

## Next steps
- [ ] Send proposal by ...
- [ ] Follow up on ...
```

Outcomes here drive the `deal.outcome_recorded` event and feed Qdrant.

Links: [[../03-agents/meeting-coach]] · [[../10-playbooks/discovery]]
