# Roadmap

Each sprint is two calendar weeks. Features reference IDs from [[PRD]].

| Sprint | Theme | Features | Exit criteria |
|---|---|---|---|
| 1 | Foundation pipeline (CLI only) | F-001, F-002, F-003, F-004, F-007 (basic) | One URL → email draft, end-to-end, on disk |
| 2 | Opportunities + dashboard | F-005, F-006, F-008, F-010 (basic), F-011, F-012 (manual), F-020 | Dashboard shows queue; first real outreach sent |
| 3 | Conversations + proposals | F-009, F-013, F-014, F-019 | First reply threaded; first proposal PDF generated |
| 4 | Discovery + sequencing | F-015, F-016, F-012 (API) | Operator can input "dental in Dubai" → 50 leads in pipeline |
| 5 | Meeting coach + memory | F-017, Qdrant integration, embedding pipeline | First meeting brief generated from past wins |
| 6 | Polish + observability | F-018, ops dashboard, public-facing demo page | Production-ready; cost-per-company tracked live |

Stretch (post-V1):
- Multi-tenant readiness (tenants table, scoped queries).
- LinkedIn outbound (Phantombuster / Apify).
- Phone-call AI (Vapi / Twilio Voice).
- Arabic outreach (translation layer + cultural prompt variants).

Links: [[PRD]] · [[features]] · [[../08-development/sprint-1]]
