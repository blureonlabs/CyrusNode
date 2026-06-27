# Agents — Overview

Every agent answers exactly one question. Specialists, not god-prompts.

The pipeline order, with the question each one answers:

| # | Agent | Question | Kind |
|---|---|---|---|
| 1 | [[lead-discovery]] | What companies match this niche? | Deterministic |
| 2 | [[crawler]] | What pages exist on this site? | Deterministic |
| 3 | [[extractor]] | What text and facts are on those pages? | Deterministic |
| 4 | [[seo-auditor]] | How does this site perform technically? | Deterministic |
| 5 | [[tech-detection]] | What stack does this site use? | Deterministic |
| 6 | [[social-intelligence]] | What is this business doing on social? | Deterministic + LLM (V2) |
| 7 | [[business-intelligence]] | What does this company actually sell, and to whom? | LLM |
| 8 | [[opportunity-finder]] | Which AI services would move the needle here? | LLM |
| 9 | [[roi-estimator]] | What is each opportunity conservatively worth? | LLM |
| 10 | [[proposal-writer]] | How do we present this as a one-page proposal? | LLM |
| 11 | [[email-writer]] | What is the most likely-to-reply cold email? | LLM |
| 12 | [[whatsapp-writer]] | What is the shortest opener that earns a reply? | LLM |
| 13 | [[meeting-coach]] | How do we walk into the call already prepared? | LLM |

Each agent file documents:
- **Input schema** (JSON Schema sketch)
- **Output schema** (JSON Schema sketch)
- **Prompt(s) used** (links into `apollo/04-prompts/`)
- **Failure modes** the worker translates to operator-visible events
- **Cost ceiling** (cents per run)
- **Acceptance criteria** for V1

Agent code lives in `crates/apollo-agents/src/<name>.rs`. The shared trait and context live in `apollo-core`. See [[../02-architecture/agent-architecture]].

---

## Patterns we keep reusing

- **Deterministic-first.** If a rule, regex, or HTML check can answer, use it. LLM only when ambiguity is irreducible.
- **Cite-or-die.** Every LLM agent's output includes an `evidence` field that points at the source string. No-evidence outputs are rejected at the schema layer.
- **Cap output tokens.** Each prompt declares its cap in frontmatter; the LLM client refuses calls that omit it.
- **Bounded numbers.** ROI, percentages, and prices have `min`/`max` in their schemas. Sanity checks prevent $10B claims.
- **Stable identifiers.** Agent NAMEs never change. Output structure changes bump VERSION.

Links: [[../02-architecture/agent-architecture]] · [[../04-prompts/_overview]] · [[../PROJECT_BIBLE]]
