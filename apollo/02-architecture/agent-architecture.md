# Agent Architecture

> One Agent trait. One context. One contract. Specialization is in the prompt, not in the framework.

**Where agents live**: each agent is an **infrastructure adapter inside its feature crate**, not a centralized "agents" crate. The `Agent` trait + worker wrapper live in `platform-queue`. The crawler adapter lives in `feature-research/src/infra/crawler_adapter.rs`. The email-writer agent lives in `feature-drafting/src/infra/email_writer_agent.rs`. See [[feature-layout]] for the full rule.

---

## 1. The trait

```rust
#[async_trait]
pub trait Agent: Send + Sync + 'static {
    const NAME: &'static str;        // stable, dotted (e.g. "crawler", "email-writer")
    const VERSION: u32;              // bump on behavior change

    type Input: DeserializeOwned + Serialize + JsonSchema + Send;
    type Output: Serialize + DeserializeOwned + JsonSchema + Send;

    fn meta() -> AgentMeta;

    async fn run(
        &self,
        ctx: &AgentContext,
        input: Self::Input,
    ) -> Result<Self::Output, AgentError>;
}
```

`AgentMeta` carries: backoff policy, concurrency permits, default timeout, cost ceiling, primary prompt name (optional, for LLM agents), and the events this agent emits on success.

---

## 2. The context

```rust
pub struct AgentContext {
    pub tenant_id: Uuid,
    pub correlation_id: String,
    pub company_id: Option<Uuid>,
    pub db: Db,                          // repository handle, scoped to tenant
    pub llm: Arc<dyn LlmClient>,         // see overview §6
    pub prompts: Arc<PromptLoader>,      // hot-reloaded
    pub crawler: Arc<dyn HttpFetcher>,   // shared connection pool
    pub embedder: Arc<dyn Embedder>,     // Qdrant + embedding client
    pub clock: Arc<dyn Clock>,           // injectable for tests
    pub cost: CostTracker,               // increment per LLM call
    pub cancel: CancellationToken,       // checked at safe points
}
```

Agents never construct any of these. The worker injects them. Tests inject fakes.

---

## 3. Two agent flavors

### Deterministic agent

No LLM call. Returns a pure function of input.

Example: `TechDetection`. Reads HTML headers + script tags, applies Wappalyzer-style rules from a YAML ruleset, returns `TechStack`. Output is reproducible, cheap, fast.

### LLM agent

Uses the prompt loader + LlmClient. Output is validated against a JSON Schema.

Example flow:

```rust
async fn run(&self, ctx: &AgentContext, input: BusinessIntelInput)
    -> Result<BusinessIntelOutput, AgentError>
{
    let prompt = ctx.prompts.get("business-summary")?;
    let rendered = prompt.render(&input)?;             // Markdown frontmatter + Tera/Handlebars body
    let resp = ctx.llm.complete(LlmRequest {
        model: prompt.frontmatter.model,
        messages: vec![user(rendered)],
        output_schema: Self::output_schema(),
        max_output_tokens: prompt.frontmatter.max_output_tokens,
        temperature: prompt.frontmatter.temperature,
        timeout: Duration::from_secs(30),
        cost_tracker: ctx.cost.clone(),
        correlation_id: ctx.correlation_id.clone(),
        agent_name: Self::NAME,
        prompt_name: prompt.name.clone(),
        prompt_version: prompt.version,
        company_id: ctx.company_id,
    }).await?;
    let output: BusinessIntelOutput = serde_json::from_value(resp.json)?;
    Ok(output)
}
```

Note what the agent **does not** do: retry on schema mismatch (LlmClient does it), log token usage (LlmClient does it), persist the artifact (worker does it), publish events (worker does it).

---

## 4. The worker wrapper (where the smarts live)

```
For each claimed job:
  1. Deserialize input.
  2. Compute input_hash (canonical JSON, SHA-256).
  3. Look up artifact (agent_name, version, input_hash).
     - HIT: publish completion event with cached artifact_id. Mark job succeeded. Done.
  4. Open tracing span. Set correlation_id, company_id, agent_name as span attrs.
  5. Acquire concurrency permit for agent_name.
  6. Start heartbeat task (60s interval).
  7. Call agent.run(ctx, input) with timeout from meta.
  8. On success:
     - Insert artifact row.
     - Insert completion event (same transaction).
     - Mark job succeeded.
  9. On error:
     - Classify (transient | terminal).
     - Transient + attempts < max: requeue with backoff.
     - Terminal or attempts maxed: mark failed, publish failure event.
 10. Stop heartbeat. Drop permit. Close span.
```

This is the **only** place these concerns live. PR review checks this is the only place.

---

## 5. Error classification

```rust
pub enum AgentError {
    /// Retriable: HTTP 5xx, timeouts, transient DB errors, LLM 429.
    Transient(anyhow::Error),

    /// Will never succeed: HTTP 404, schema mismatch after retry, bad input.
    Terminal(anyhow::Error),

    /// Operator-relevant input problem (e.g. site blocks scraping).
    Domain(DomainError),
}
```

The worker:
- `Transient` → backoff + requeue (if attempts left).
- `Terminal` → mark failed terminally.
- `Domain` → mark failed terminally **and** publish a domain-specific event (e.g., `crawl.blocked`) the operator can see in the dashboard.

---

## 6. Conventions per agent

- One file per agent: `crates/apollo-agents/src/<name>.rs`.
- Module exports `pub struct <Name>Agent; impl Agent for <Name>Agent`.
- Input/Output structs colocated and labeled `#[derive(Serialize, Deserialize, JsonSchema)]`.
- Public docstring on the impl block describes: purpose, inputs, outputs, prompt(s) used, cost ceiling, emitted events.
- Unit tests verify deterministic logic.
- Golden tests live under `apollo/09-experiments/<agent>/`.

---

## 7. Registry

```rust
pub struct AgentRegistry {
    by_name: HashMap<&'static str, Arc<dyn ErasedAgent>>,
}

impl AgentRegistry {
    pub fn install<A: Agent>(&mut self, agent: A) { ... }
    pub fn get(&self, name: &str) -> Option<Arc<dyn ErasedAgent>> { ... }
}
```

The worker constructs the registry at startup, installing every agent. Adding a new agent = one line in `register_agents()` + the agent file.

---

## 8. Cancellation discipline

Agents check `ctx.cancel.is_cancelled()` at every "safe point" — between sub-requests, between page fetches, before/after each LLM call. On cancellation:

```rust
if ctx.cancel.is_cancelled() {
    return Err(AgentError::Terminal(anyhow!("cancelled")));
}
```

LLM calls inherit cancellation through `tokio::select!` in `LlmClient::complete`.

---

## 9. Cost ceilings

Each agent declares an expected cost ceiling per run, in cents:

```rust
fn meta() -> AgentMeta {
    AgentMeta { cost_ceiling_cents: 2, ..default() }
}
```

The `CostTracker` accumulates per call. If a single run exceeds the ceiling, the worker:
1. Lets the current call complete (no half-charges).
2. Returns `Terminal(CostCeilingExceeded)` rather than continuing.
3. Operator sees the failure in the dashboard with the exact agent + prompt + tokens.

Ceilings are deliberately tight. They're how we notice prompt regressions before the monthly bill does.

---

## 10. Composition (sagas vs. orchestration)

Agents NEVER call other agents directly. If agent A needs agent B's output:

- A is enqueued only when B has emitted its completion event, **OR**
- A reads B's most recent artifact via the repository.

Cross-agent orchestration happens at the saga layer. Agents stay one-step.

---

Links: [[overview]] · [[event-driven]] · [[queue-system]] · [[../03-agents/_overview]] · [[../04-prompts/_overview]]
