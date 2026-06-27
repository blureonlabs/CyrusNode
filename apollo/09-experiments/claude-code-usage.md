# Using Claude Code as part of the Apollo workflow

> Three modes. Two are for the operator's dev workflow. One is overkill for V1 but interesting for V3.

The Anthropic API (used directly via `platform-llm`) is what powers Apollo's per-company LLM calls. Claude Code is **not** that — Claude Code is an *agent* on top of the API. Use it for **developer leverage**, not runtime.

---

## Mode 1 — Claude Code as your IDE pair (default)

Just run `claude` in the project root. Claude Code reads `CLAUDE.md` automatically. Use this for:

- Sprint-by-sprint implementation (one story per session).
- Refactors that span features.
- Reviewing diffs before commit.

This is where 90% of the time is spent.

---

## Mode 2 — Claude Code GitHub Action

Repo: `anthropics/claude-code-action`. Available on paying Claude plans. Triggered by `@claude` mentions in issues or PR comments.

Examples that fit Apollo's dev workflow:
- "@claude implement S1-T05" in an issue → opens a PR.
- "@claude fix the clippy warnings" in a PR → pushes a fixup commit.
- "@claude review for the dependency rule" → comments on violations of the `domain/` import rule.

Setup (one-time, when you're ready):
1. Install the GitHub App on the repo.
2. Add `ANTHROPIC_API_KEY` as a repo secret.
3. Drop the workflow YAML the action installer suggests into `.github/workflows/`.

**Don't use it for runtime tasks** (e.g. "research this lead"). The Action is bound to PR/issue events.

---

## Mode 3 — Claude Code headless (`claude -p`)

Shell-invokable. Returns stdout. Examples:

```bash
claude -p "Audit goldens in apollo/09-experiments/prompts/ — list ones whose actual vs expected differ by more than 10% by token count" > out.md
```

Useful for scheduled background tasks:
- Nightly: re-run goldens against current prompts, open a PR if drift exceeds a threshold.
- Weekly: audit `apollo/10-playbooks/objection-handling.md` against logged objections from `deal.outcome_recorded`, propose new entries.
- Pre-meeting: enrich the Meeting Coach brief with a Claude-Code-style web search if the prospect has news in the last 30 days.

Cost note: headless runs are expensive per invocation (tool use loops + full context). Use sparingly. Apollo's per-company LLM calls go through the Anthropic API directly via `platform-llm`, not through Claude Code.

---

## Mode 4 (later) — Claude Agent SDK

The Python/TypeScript SDK lets you build Claude-Code-style agents in your own service. Tool use, file editing, multi-step planning.

Plausible Apollo use case (V3):
- **"Deep research" mode** for high-value prospects: an agent that browses the web, reads competitor press releases, finds case studies, and writes a paragraph for the Meeting Coach brief. Worth $0.50–$2 per run only for prospects with high deal-size signals.

Not in scope for V1–V2. Recorded here so we don't reinvent it.

---

## Decision matrix

| Job | Use |
|---|---|
| Per-company LLM call (any agent) | Anthropic API via `platform-llm` |
| Implement a sprint story | Claude Code interactive |
| Fix CI lint / open a PR from an issue | Claude Code GitHub Action |
| Scheduled audit / cleanup / drift detection | `claude -p` headless |
| Deep-research a single named prospect | Claude Agent SDK (V3) |

Links: [[../DECISIONS]] · [[../CLAUDE]] · [[../02-architecture/agent-architecture]]
