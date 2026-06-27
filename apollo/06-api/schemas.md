# Shared JSON Schemas

> Schemas live close to what they describe. This page is the index.

## Where things live

| Kind | Location |
|---|---|
| Prompt output schemas | `apollo/04-prompts/schemas/<name>.v<n>.json` |
| Event payload schemas | `apollo/06-api/event-schemas/<type>.v<n>.json` |
| REST request/response schemas | Auto-generated from Rust via `utoipa`; published at `/openapi.json` |
| Agent input/output schemas | Auto-generated from Rust via `schemars`; checked into `apollo/04-prompts/schemas/` |

## Generation flow

Rust types are the source of truth. CI runs:

1. `cargo run -p apollo-tools -- generate-schemas` — writes prompt output schemas + OpenAPI to disk.
2. `git diff --exit-code` — fails if a generated file changed without commit. Force the developer to commit the regenerated artifacts.

## Versioning

- Each schema has a version in its filename: `business-summary.v1.json`, `crawl.completed.v1.json`.
- Bumps require an upcaster (events) or schema migration plan (prompt outputs — usually trivial because we only read newest).
- See [[../02-architecture/event-driven]] §8.

Links: [[rest]] · [[events]] · [[../04-prompts/_overview]]
