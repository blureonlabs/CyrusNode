# Prompt output schemas

One JSON Schema per `(prompt_name, version)`. Filename: `<name>.v<n>.json`.

The LLM client validates every response against the schema. Mismatch → one re-prompt with the error → terminal.

Schemas live next to prompts so a prompt-and-schema change ships in one commit. When you bump `version:` in the prompt frontmatter, copy the schema to `<name>.v<n+1>.json` and edit there. Do not edit old schema files.

Files to add when implementing each prompt:
- `business-summary.v1.json`
- `opportunity-finder.v1.json`
- `roi-estimator.v1.json`
- `proposal-writer.v1.json`
- `email-writer.v1.json`
- `whatsapp-writer.v1.json`
- `meeting-coach.v1.json`

Use `schemars` in Rust to auto-generate the schema from the agent's `Output` type during Sprint 1, then check the generated file in here.

Links: [[../_overview]] · [[../../03-agents/_overview]]
