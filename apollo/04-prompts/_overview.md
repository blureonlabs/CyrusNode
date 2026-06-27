# Prompts — Overview

> Prompts are data. Markdown files. Hot-reloadable. Versioned. Never inlined in Rust.

---

## 1. File shape

Every prompt is a Markdown file with frontmatter:

```markdown
---
name: business-summary
version: 3
model: gemini-2.5-flash
temperature: 0.2
max_output_tokens: 1500
inputs:
  - business_intel_context        # rendered via Tera
outputs_schema: schemas/business-summary.v3.json
description: >
  Produces a 5-field business summary from extracted website content.
---

You are a senior B2B analyst. Read the following compiled context and produce
a structured business summary with evidence...

# Context
{{ business_intel_context }}

# Output rules
- Cite at least three pieces of evidence.
- ...
```

The loader parses frontmatter, registers the prompt in the `prompt_versions` table, and serves it via `PromptLoader.get(name)`.

---

## 2. Versioning rules

- Bump `version:` whenever the prompt body or frontmatter changes in a way that could change outputs.
- The loader refuses to serve a prompt whose body SHA-256 differs from the recorded version's hash. Fix: bump the version.
- Old versions remain in history. `prompt_versions` table records every (name, version) ever observed, with body hash.
- Artifacts record `prompt_version` so we can replay any old output reproducibly.

---

## 3. Schemas

Each prompt has a JSON Schema under `apollo/04-prompts/schemas/<name>.v<n>.json`. The LLM client validates output against the schema on every call. Mismatch → one re-prompt with the error message → terminal.

Schemas live next to the prompts so a prompt change + schema change ships together.

---

## 4. Goldens

Every prompt has a golden test set under `apollo/09-experiments/prompts/<name>/`:

```
<name>/
  fixtures/
    case-01.input.json
    case-01.expected.json
    ...
  README.md
```

Goldens run as part of `cargo test --features prompt-goldens` and are NOT run in CI by default (LLM cost). They are run manually before merging a prompt change. Diffs against `expected.json` must be reviewed and committed if accepted.

---

## 5. Library (V1)

| Prompt | Used by |
|---|---|
| [[business-summary]] | Business Intelligence agent |
| [[opportunity-finder]] | Opportunity Finder agent |
| [[roi-estimator]] | ROI Estimator agent |
| [[proposal-writer]] | Proposal Writer agent |
| [[email-writer]] | Email Writer agent |
| [[whatsapp-writer]] | WhatsApp Writer agent |
| [[meeting-coach]] | Meeting Coach agent (V5) |

---

## 6. Tone modules

Industry-specific tone subsections live in `apollo/05-knowledge/industries/<industry>.md` under a `## Tone` heading. The Email Writer + WhatsApp Writer prompts pull these via `{{ tone }}` and concatenate at render time.

Adding a new industry's tone module is a docs change — no code touched.

---

Links: [[../03-agents/_overview]] · [[../05-knowledge/]] · [[../CLAUDE]]
