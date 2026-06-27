# Feature-Driven DDD — Layout

> Apollo is organized **by feature, not by layer**. This file is the rulebook for what goes where.

---

## 1. Why

Layer-based projects (one giant `controllers/`, one `models/`, one `services/`) force you to touch five folders to change one feature. Feature-based projects keep everything for a capability in one folder. Once you've read one feature, you've read them all.

That's the whole pitch. Everything below is the discipline that makes it stick.

---

## 2. The module map

Apollo is **one crate** (`name = "apollo"`). Modules under `src/` hold the structure. Binaries under `src/bin/` are thin entry points.

```
src/
  lib.rs
  bootstrap.rs            # the only file that knows about all features

  bin/
    apollo-api.rs         # Axum HTTP server
    apollo-worker.rs      # queue consumer
    apollo-cli.rs         # operator CLI

  platform/               # shared kernel
    core/                 # shared base types (Tenant, Ids, Clock)
    db.rs                 # Postgres pool + migrations runner
    queue.rs              # jobs table, SKIP LOCKED, Agent trait
    events.rs             # event bus + saga runtime
    llm.rs                # LlmClient trait + Gemini + Claude impls
    prompts.rs            # hot-reloading prompt loader
    crawler.rs            # HTTP/headless fetch + HTML→Markdown
    observability.rs      # tracing setup

  features/
    discovery/
    research/
    synthesis/
    drafting/
    review/
    outreach/
    conversations/
    meetings/
    intelligence/
```

**Platform vs. feature:** `platform::*` modules are *capabilities the product needs from the world* — talk to Postgres, talk to an LLM, run a job. They don't know what Apollo does. `features::*` modules are *what Apollo does* — research a company, draft outreach, run a meeting. They use platform but never reach into another feature.

---

## 3. The four boxes (inside every feature)

```
src/features/<name>/
  mod.rs
  configure.rs       # wires the four boxes for this feature
  domain/            # types + rules. zero I/O deps.
  application/       # use cases (workflows)
  infra/             # adapters that implement domain's ports
  presentation/      # HTTP routes + event subscriptions
```

### domain/ — "What is this thing and what are its rules?"

Pure types and pure functions.

- Allowed: `serde`, `thiserror`, `chrono`, `uuid`, `async-trait` (for async port traits), `crate::platform::core`.
- Forbidden: `sqlx`, `reqwest`, `axum`, `tokio::net`, `tokio::fs`, anything in `crate::platform::{db,llm,crawler,events,queue,prompts}`, any other feature's modules.
- Contains: entities, value objects, port traits, domain errors, business rules.

A port trait sits in `domain/ports.rs`. Example:

```rust
#[async_trait]
pub trait CrawlPort: Send + Sync {
    async fn fetch_site(&self, url: &Url, budget: CrawlBudget) -> Result<CrawlResult, CrawlError>;
}
```

The application asks for this port. Infrastructure implements it.

### application/ — "What can a user do with it?"

Use cases. One file per use case. The use case orchestrates the workflow using domain types and ports.

```rust
pub struct ResearchCompany {
    crawl: Arc<dyn CrawlPort>,
    extract: Arc<dyn ExtractPort>,
    seo: Arc<dyn SeoPort>,
    bizintel: Arc<dyn BizIntelPort>,
    events: Arc<dyn EventPublisher>,
}

impl ResearchCompany {
    pub async fn run(&self, input: ResearchInput) -> Result<DossierBase, AppError> {
        // workflow: crawl → extract → audit (parallel) → bizintel
        // no SQL, no HTTP, no JSON, no LLM SDK calls — just ports.
    }
}
```

- Allowed deps: `domain`, `platform-core`, port traits from other features (if absolutely necessary — rare).
- Forbidden: any I/O module (`platform::db`, `platform::llm`, etc.). Use ports.

### infra/ — "How do we talk to the outside world?"

Adapters that implement the ports. This is where actual SQL, HTTP, LLM SDK calls, Chromium spawns, S3 puts, and so on live.

```rust
pub struct PostgresCrawlRepo { pool: PgPool }

#[async_trait]
impl CrawlPort for PostgresCrawlRepo {
    async fn fetch_site(&self, url: &Url, budget: CrawlBudget) -> Result<CrawlResult, CrawlError> {
        // uses sqlx, platform-crawler, etc.
    }
}
```

- Allowed deps: `domain`, `platform-*`, third-party SDKs (sqlx, reqwest, aws-sdk-s3, …).
- Forbidden: `application`, `presentation`. Infra fills domain's ports; it doesn't know who's asking.

### presentation/ — "How does the outside world talk to us?"

HTTP handlers, event subscriptions, CLI commands for this feature. Thin. Parse → validate → call application → serialize.

```rust
async fn create_company(
    State(uc): State<Arc<ResearchCompany>>,
    Json(body): Json<CreateCompanyBody>,
) -> Result<Json<CompanyView>, ApiError> {
    let result = uc.run(body.into()).await?;
    Ok(Json(result.into()))
}
```

- Allowed deps: `application`, `domain`, `platform-core`, `axum`, request/response DTOs.
- Forbidden: `infra`. Never call an adapter directly; go through `application`.

---

## 4. The wiring file (`configure.rs`)

The only file in each feature that knows about all four boxes. It builds the adapters, hands them to the application, and exposes a single function the bootstrap calls.

```rust
pub struct ResearchModule {
    pub routes: axum::Router<()>,
    pub subscriptions: Vec<EventSubscription>,
}

pub fn configure(deps: PlatformDeps) -> ResearchModule {
    // build adapters
    let crawl = Arc::new(PostgresCrawlRepo::new(deps.db.clone()));
    let extract = Arc::new(MarkdownExtractor::new());
    let seo = Arc::new(LighthouseRunner::new(deps.chromium.clone()));
    let bizintel = Arc::new(BizIntelAgent::new(deps.llm.clone(), deps.prompts.clone()));

    // build use case
    let research = Arc::new(ResearchCompany::new(crawl, extract, seo, bizintel, deps.events.clone()));

    // build presentation
    let routes = presentation::routes(research.clone());
    let subscriptions = presentation::subscriptions(research);

    ResearchModule { routes, subscriptions }
}
```

`PlatformDeps` is a small struct of handles to platform crates (DB pool, LLM client, prompt loader, event publisher, etc.) constructed once at boot.

---

## 5. The bootstrap (one file per app)

Each app has exactly one bootstrap file (`apps/apollo-api/src/main.rs`, etc.) that:

1. Loads config and secrets.
2. Builds platform handles (DB pool, LLM client, prompt loader, event bus, …).
3. Calls each feature's `configure(deps)` and collects routes/subscriptions.
4. Mounts routes onto Axum; registers subscriptions with the event bus.
5. Starts serving.

Bootstrap is the **only file** that knows about all features. Features don't know about each other.

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let cfg = load_config()?;
    let deps = PlatformDeps::build(&cfg).await?;

    let discovery   = feature_discovery::configure(deps.clone());
    let research    = feature_research::configure(deps.clone());
    let drafting    = feature_drafting::configure(deps.clone());
    let review      = feature_review::configure(deps.clone());
    let outreach    = feature_outreach::configure(deps.clone());
    let convos      = feature_conversations::configure(deps.clone());
    let meetings    = feature_meetings::configure(deps.clone());

    let app = axum::Router::new()
        .merge(discovery.routes)
        .merge(research.routes)
        .merge(drafting.routes)
        .merge(review.routes)
        .merge(outreach.routes)
        .merge(convos.routes)
        .merge(meetings.routes)
        .with_state(deps.clone());

    deps.events.subscribe_all([
        discovery.subscriptions,
        research.subscriptions,
        drafting.subscriptions,
        // ...
    ]).await?;

    axum::serve(listener, app).await?;
    Ok(())
}
```

---

## 6. The dependency rule (the only test)

```
presentation → application → domain ← infra
                                ↑
                         platform-* (any layer can use platform, except domain restricts to platform-core)
```

A CI lint enforces it. Easiest mechanism: a small `cargo deny` config + a custom check that scans each `feature-*/src/domain/` for forbidden imports.

If `domain/` ever imports `reqwest`, `sqlx`, `axum`, or `tokio::net`, the build fails.

---

## 7. When NOT to make a port

- A type used only inside one file? No port. Use the concrete type.
- A function called once at boot? No port. Use a factory function.
- A "just in case we swap it later" port that has one implementation? **Especially** no port. Add it when the second implementation actually arrives.

Ports cost code volume and indirection. Earn them.

---

## 8. Cross-feature communication

Features never `use feature_other_thing::…`. Two allowed channels:

1. **Events.** Feature A publishes; Feature B subscribes. Schema in `apollo/06-api/events.md`.
2. **Shared types in `platform-core`.** Only for genuinely universal IDs and value objects (`CompanyId`, `TenantId`, `Money`, `Email`). Not for feature-specific entities.

If you find yourself wanting Feature A to call Feature B directly: write an event, or pull the shared concern into `platform-core`.

---

## 9. Testing per feature

- `domain` tests: instantaneous, no async, no I/O. Just pure logic. `cargo test -p feature-research domain::` runs in milliseconds.
- `application` tests: substitute ports with fakes (`pub struct FakeCrawlPort;`). Run the use case end-to-end without touching the network.
- `infra` tests: testcontainers Postgres, recorded HTTP fixtures, mocked LLM. These are the slow ones.
- `presentation` tests: spin up Axum with stubbed use cases; assert HTTP shapes.

Each feature's tests run independently. `cargo test -p feature-outreach` doesn't touch the research crate.

---

## 10. Adding a new feature

1. `cargo new --lib crates/feature-newthing`.
2. Add the four boxes. Even if some are empty, keep the shape.
3. Write `configure.rs`. Expose a `pub fn configure(deps: PlatformDeps) -> NewthingModule`.
4. Update the bootstrap in `apps/apollo-api/src/main.rs` (and worker/cli as needed) to call it.
5. Add events to `apollo/06-api/events.md` if the feature emits any.
6. Done. No other feature changes.

---

## 11. The day-to-day question

When you're about to write code, ask:

- "Is this a fact about my world?" → `domain/`
- "Is this a step in a user's workflow?" → `application/`
- "Does this talk to anything external?" → `infra/`
- "Is this how the outside calls in?" → `presentation/`

Then the file's location answers itself.

Links: [[overview]] · [[agent-architecture]] · [[event-driven]] · [[../DECISIONS]]
