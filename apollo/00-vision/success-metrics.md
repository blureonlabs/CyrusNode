# Success Metrics

Apollo's success is measurable at three levels: pipeline, product, and platform.

## Pipeline (operator outcome)

| Metric | V1 target | V3 target |
|---|---|---|
| Researched companies / week | 50 | 500 |
| Outreach sent / week (post-review) | 30 | 250 |
| Reply rate | ≥ 5% | ≥ 12% |
| Booked meetings / month | 5–10 | 40+ |
| Meeting → proposal rate | ≥ 50% | ≥ 60% |
| Proposal → close rate | ≥ 25% | ≥ 35% |

## Product (system performance)

| Metric | Target |
|---|---|
| URL → finished dossier latency (p50) | ≤ 90s |
| URL → finished dossier latency (p95) | ≤ 240s |
| Cost per researched company | ≤ $0.05 |
| Crawler success rate | ≥ 95% |
| Schema validation pass rate (LLM output) | ≥ 98% on first attempt |
| Goldens stable across prompt revisions | 100% must be reviewed before merge |

## Platform (operator experience)

| Metric | Target |
|---|---|
| Operator time per outreach (review → send) | ≤ 2 minutes |
| Operator can replay a failed pipeline | one click |
| Operator can see cost-per-company live | yes, dashboard |
| New industry playbook → first researched company | ≤ 30 minutes |

The cost number is the watchdog. If cost per company exceeds $0.10, we stop adding features and fix it. If reply rate drops below 5%, we stop adding features and fix the prompts.

Links: [[vision]] · [[mission]] · [[business-model]]
