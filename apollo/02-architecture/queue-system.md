# Queue System

> Postgres-backed job queue with SKIP LOCKED and leases. Boring on purpose.

---

## 1. Why Postgres, not Redis or SQS

- We already need Postgres for everything else. Adding Redis means one more thing to operate, monitor, and back up.
- Job state is part of the audit trail. Putting it in the same database as `events` lets us write jobs and events in the same transaction.
- SKIP LOCKED + a small index is enough for V1 throughput (target: tens of jobs per second).
- We can migrate to Redis Streams or NATS in V2 without rewriting agents — see [[../DECISIONS]] ADR-005.

---

## 2. Job lifecycle

```
   queued ──► leased ──► running ──► succeeded
                             │
                             ├──► failed (attempts < max → back to queued)
                             │
                             └──► failed (terminal, attempts >= max)

   queued ──► cancelled  (operator action)
```

- `queued` — waiting. `available_at` controls earliest pickup (for delayed jobs).
- `leased` — a worker claimed it but hasn't started yet (rare; just the transition window).
- `running` — worker is executing.
- `succeeded` — terminal. Artifact written. Completion event published.
- `failed` (terminal) — terminal. `last_error` populated. Failure event published.
- `cancelled` — operator hit Stop. No event.

---

## 3. Claiming a job (the only safe query)

```sql
WITH claimed AS (
    SELECT id
    FROM jobs
    WHERE status = 'queued'
      AND available_at <= now()
    ORDER BY priority ASC, available_at ASC
    LIMIT $1                                -- batch size, usually 1–4
    FOR UPDATE SKIP LOCKED
)
UPDATE jobs SET
    status        = 'running',
    leased_until  = now() + interval '5 minutes',
    started_at    = COALESCE(started_at, now()),
    attempt_count = attempt_count + 1
FROM claimed
WHERE jobs.id = claimed.id
RETURNING jobs.*;
```

Properties:

- **SKIP LOCKED** means concurrent workers never block each other; each picks distinct rows.
- **Atomic**: claim + state transition in one statement.
- **Lease-based**: `leased_until` is the worker's heartbeat. If it expires, another worker reclaims.

---

## 4. Heartbeating

Long-running jobs (crawler, headless Chromium) update their lease every 60 seconds:

```sql
UPDATE jobs SET leased_until = now() + interval '5 minutes'
WHERE id = $1 AND status = 'running';
```

If the worker crashes, no heartbeat → lease expires within 5 minutes → reaper reclaims.

---

## 5. The reaper

A worker task running every 30 seconds:

```sql
UPDATE jobs SET
    status = 'queued',
    leased_until = NULL,
    last_error = COALESCE(last_error, 'lease expired')
WHERE status = 'running'
  AND leased_until < now()
  AND attempt_count < max_attempts;
```

Jobs that exceeded `max_attempts` go terminal:

```sql
UPDATE jobs SET
    status = 'failed',
    finished_at = now()
WHERE status = 'running'
  AND leased_until < now()
  AND attempt_count >= max_attempts;
```

The reaper publishes `job.reclaimed` events for visibility.

---

## 6. Enqueue (with idempotency)

```rust
pub async fn enqueue<A: Agent>(
    db: &Db,
    correlation_id: &str,
    company_id: Uuid,
    input: A::Input,
) -> Result<Uuid> {
    let canonical = serde_json::to_vec(&input)?;  // sorted-key canonical form
    let input_hash = sha256_hex(&canonical);

    // ON CONFLICT DO NOTHING due to unique (tenant, agent, version, input_hash)
    let row = sqlx::query!(
        r#"INSERT INTO jobs
            (tenant_id, agent_name, agent_version, company_id, correlation_id, input, input_hash)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           ON CONFLICT (tenant_id, agent_name, agent_version, input_hash) DO UPDATE
             SET correlation_id = jobs.correlation_id
           RETURNING id, status"#,
        tenant_id, A::NAME, A::VERSION, company_id, correlation_id, canonical, input_hash
    ).fetch_one(db).await?;

    Ok(row.id)
}
```

If the same `(agent, version, input)` was already done, we return the existing job and downstream code re-uses the cached artifact. No work is repeated.

---

## 7. Backoff

Per-agent default in `AgentMeta`:

```rust
pub struct BackoffPolicy {
    pub base_ms: u64,
    pub max_ms: u64,
    pub multiplier: f64,
    pub jitter: bool,
}
```

On failure, the worker computes the next `available_at` as `now() + min(max_ms, base_ms * multiplier^attempt) + jitter` and sets `status = 'queued'`. The reaper handles crashes; explicit failures use this code path.

Defaults: `base_ms = 2_000`, `multiplier = 3.0`, `max_ms = 300_000`. Override per agent (the Crawler uses longer; the LLM agents use shorter).

---

## 8. Priorities

`priority INT NOT NULL DEFAULT 100`. Lower runs first.

Conventions:

| Priority | Used by |
|---|---|
| 10 | Operator-initiated replays |
| 50 | Pre-meeting briefs |
| 100 | Standard research pipeline |
| 200 | Bulk imports (discovery dumps) |

---

## 9. Concurrency

The worker has a Tokio semaphore per agent name:

```rust
pub struct AgentConcurrency {
    pub crawler: usize,           // 8
    pub seo: usize,               // 4 (Chromium is heavy)
    pub llm_default: usize,       // 16
    pub llm_pro: usize,           // 4 (Gemini Pro rate limits)
}
```

Workers claim only as many jobs of each kind as their permits allow.

---

## 10. Cancellation

The dashboard's Stop button updates `status = 'cancelled'`. The worker checks `status` at every heartbeat; if cancelled, it aborts the agent task and writes a `job.cancelled` event.

---

## 11. Observability

- Counter: `jobs_claimed_total{agent}`
- Counter: `jobs_completed_total{agent,outcome}`
- Histogram: `job_duration_ms{agent}`
- Gauge: `jobs_queued{agent}` (from the `pipeline_health` view)

Grafana dashboard mock in [[../09-experiments/dashboards.md]] (stub until V1.6).

---

Links: [[overview]] · [[event-driven]] · [[database]] · [[agent-architecture]]
