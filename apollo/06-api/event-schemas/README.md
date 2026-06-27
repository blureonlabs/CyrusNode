# Event schemas

One JSON Schema per (event type, version). Filename: `<type>.v<n>.json`.

E.g. `crawl.completed.v1.json`, `bizintel.completed.v1.json`.

Schemas are generated from Rust event payload structs via `schemars` during Sprint 1. CI runs `cargo run -p apollo-tools -- generate-schemas` and fails on drift. Treat these files as build artifacts that are checked in for review.

Links: [[../events]] · [[../../02-architecture/event-driven]]
