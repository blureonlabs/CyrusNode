# ER Diagram

```
                          ┌──────────┐
                          │ tenants  │
                          └────┬─────┘
                               │ 1..*
                               ▼
                       ┌───────────────┐
                       │   companies   │◄──────────────┐
                       └────┬──────────┘                │
                            │                           │
                ┌───────────┼───────────┐              │
                ▼           ▼           ▼              │
            ┌────────┐  ┌────────────┐  ┌────────┐    │
            │contacts│  │conversations│  │ deals  │    │
            └────────┘  └─────┬──────┘  └────────┘    │
                              │                         │
                              ▼                         │
                         ┌────────┐                     │
                         │messages│                     │
                         └───┬────┘                     │
                             │                          │
                             ▼ artifact_id (optional)   │
                       ┌──────────────┐                 │
                       │  artifacts   │                 │
                       └──────┬───────┘                 │
                              │                          │
                              ▼                          │
                       ┌──────────────┐                  │
                       │  llm_calls   │                  │
                       └──────────────┘                  │
                                                         │
        ┌──────┐ correlation_id ─► ┌──────────┐         │
        │ jobs │ ─────────────────►│  events  │─────────┘ company_id
        └──────┘                   └──────────┘
                                         ▲
                                         │
                                 ┌───────┴───────┐
                                 │  saga_state   │
                                 └───────────────┘
```

(Render as Mermaid in Obsidian — placeholder ASCII for now.)

```mermaid
erDiagram
    tenants ||--o{ companies : owns
    companies ||--o{ contacts : has
    companies ||--o{ conversations : has
    conversations ||--o{ messages : contains
    companies ||--o{ deals : has
    companies ||--o{ artifacts : produces
    artifacts ||--o{ messages : sourced_from
    artifacts ||--o{ llm_calls : derived_from
    companies ||--o{ events : about
    jobs ||--o{ events : emits
```

Links: [[schema]] · [[../02-architecture/database]]
