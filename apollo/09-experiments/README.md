# Experiments

This folder holds prompt goldens, model benchmarks, and cost-optimization runs.

## Layout
```
09-experiments/
  prompts/
    <prompt-name>/
      fixtures/                    # input fixtures + expected outputs
      README.md                    # what we're testing
  models/
    flash-vs-pro-bizintel.md       # benchmark report
  cost/
    sprint-1-cost-runs.md          # cost-per-company measurements
```

## Conventions
- Every prompt under `04-prompts/` has a matching folder here.
- Fixtures are `.input.json` + `.expected.json`.
- Reports are Markdown with the date, configuration, and a results table.
- A prompt version bump requires re-running goldens and committing diffs.

Links: [[../04-prompts/_overview]] · [[../00-vision/success-metrics]]
