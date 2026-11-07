# research-station architecture

**Status:** Phase 2 pipeline, worker, and daemon complete; HTTP API and MCP remain.
**Date:** 2026-11-04

---

## 1. Purpose

`research-station` replaces web-only "research mode" with a local, durable research system:

- a Rust daemon that owns search jobs, rate limits, budgets, evidence, and artifacts;
- an HTTP API, a CLI, and an MCP server as thin clients;
- research output stored as reproducible bundles under `research/`;
- a dendritic Nix flake for builds, development, and service deployment.

The first provider is Tavily. The provider boundary is intentionally generic so Kagi, Brave, or a self-hosted SearXNG can be added without changing the pipeline.

---

## 2. System shape

```
┌──────────────┐   HTTP/JSON     ┌─────────────────────────────────────┐
│ research CLI │────────────────▶│ researchd                           │
├──────────────┤                 │  ├─ API (axum)                      │
│ research-mcp │──stdio or HTTP─▶│  ├─ Job service: queue, leases,     │
└──────────────┘                 │  │   cancellation, resume           │
                                 │  ├─ Pipeline: plan → collect →      │
                                 │  │   evidence → synthesize → audit  │
                                 │  ├─ Provider pool: Tavily first,    │
                                 │  │   rate limit, cache, budget      │
                                 │  └─ SQLite + content-addressed blobs│
                                 └─────────────────────────────────────┘
```

One daemon process. The API is the contract. CLI and MCP translate transport into API calls; neither contains research logic.

---

## 3. Repository layout

```
research-station/
├── architecture.md
├── flake.nix                      # dendritic root
├── justfile
├── secretspec.toml                # declared credentials, no values
├── nix/modules/                   # one self-contained flake-parts module per concern
├── research/                      # research OUTPUT, one directory per project
│   ├── INDEX.md
│   └── <date>-<slug>/
│       ├── brief.md
│       ├── plan.v1.json
│       ├── plan.v1.md
│       ├── queries.jsonl
│       ├── sources.jsonl
│       ├── notes/
│       ├── report.md
│       ├── audit.md
│       └── run.json
├── research-daemon/               # the agent
│   ├── Cargo.toml                 # workspace
│   ├── rust-toolchain.toml
│   ├── config/researchd.example.toml
│   ├── migrations/
│   └── crates/
│       ├── research-core/         # domain types, config, errors; no I/O
│       ├── research-store/        # SQLite cache, blob store, later job store
│       ├── research-provider/     # SearchProvider trait, Tavily, Reddit, limiter, budget
│       ├── research-llm/          # OpenAI-compatible model client, fake, usage
│       ├── research-agent/        # budgeted model-driven plan/search/synthesis loop
│       ├── research-pipeline/     # later: plan, collect, evidence, synthesis, audit
│       ├── research-service/      # later: jobs, leases, workers
│       ├── research-api/          # later: axum surface
│       ├── research-cli/          # `research` binary
│       ├── research-mcp/          # later: MCP adapter
│       ├── research-daemon/       # later: `researchd` binary
│       └── research-testkit/      # later: fakes and fixtures
└── docs/
```

Dependency direction is one-way: `core` ← `store`/`provider` ← `pipeline` ← `service` ← surfaces/binary. `core` knows nothing about HTTP, SQL, or Tavily.

---

## 4. Domain model

Stable IDs and immutable records:

```rust
JobId, PlanId, SubquestionId, QueryId, SourceId, PassageId, ClaimId
Depth = Quick | Thorough | Exhaustive
JobStatus = Queued | Planning | AwaitingReview | Running
          | Synthesizing | Auditing | Completed
          | Failed | Cancelled | Interrupted
```

Core records:

| Record | Fields |
|---|---|
| `ResearchRequest` | question, depth, source constraints, budget, output dir, review policy |
| `Plan` | raw topic, sharpened topic, 4–8 subquestions, constraints, coverage suggestions, budget; immutable and versioned |
| `Subquestion` | question, source class, query seeds, success criterion |
| `QueryRecord` | provider, text, normalized params, timestamp, status, result count, latency, credits, raw hash |
| `Source` | URL, canonical URL, title, publisher, published date, retrieved date, source type, content hash, quality rationale, independence group |
| `Passage` | source ID, exact text, locator, content hash |
| `Claim` | text, evidence IDs, confidence, support status |

SQLite owns job state, plans, queries, evidence indexes, usage, and cache metadata. Large raw responses and extracted text live in a content-addressed blob store. The research bundle keeps hashes and, with `--keep-raw`, copies of the raw material.

---

## 5. Job state machine

- Durable queue in SQLite. Workers claim with `UPDATE ... WHERE status='queued' RETURNING`, take a lease, and heartbeat.
- On startup, expired leases become `Interrupted`. `research resume` requeues unfinished subquestions and preserves completed ones.
- Each job has `max_queries`, `max_sources`, `max_llm_tokens`, `max_wall_time`, `max_cost`. Budget is reserved before a request.
- Cancellation is a per-job token, persisted as `Cancelled`.
- Global caps: `max_concurrent_jobs`, `max_in_flight_queries`, `max_in_flight_per_job`. Every fan-out uses a bounded `JoinSet`; no unbounded `gather`.

---

## 6. Provider layer

```rust
#[async_trait]
pub trait SearchProvider: Send + Sync {
    fn id(&self) -> &'static str;
    async fn search(&self, request: SearchRequest)
        -> Result<ProviderResponse<SearchResponse>, ProviderError>;
    async fn extract(&self, request: ExtractRequest)
        -> Result<ProviderResponse<ExtractResponse>, ProviderError>;
}
```

Normalized request fields: objective, keywords, max results, depth, topic, include/exclude domains, time range, start/end date, raw content, published-date handling, country.

`ProviderPool` wraps each provider with:

- **Token bucket** for requests/minute and burst.
- **Concurrency semaphore** per provider.
- **Retry policy**: exponential backoff with jitter, honor `Retry-After`, retry only 429/5xx/timeouts and capacity-exceeded responses.
- **Circuit breaker** after repeated failures (later phase).
- **Query cache** keyed by `sha256(provider + operation + normalized request)`, TTL in SQLite.
- **Budget reservation** shared across concurrent workers.
- **Raw response persistence** as a content-addressed blob.

Tavily implementation:

- `POST /search` and `POST /extract`, bearer auth.
- `Quick → basic`, `Thorough/Exhaustive → advanced`; `topic=news` and date filters where freshness matters.
- `include_usage: true` so credits are recorded.
- `432`/`433` are quota/plan errors; `434` is capacity; `429` honors `Retry-After`.
- Limits are configuration, never hard-coded.

Reddit is a separate backend and agent action, not a Tavily topic:

- `reddit_search` is available to the agent when the question is explicit about
  Reddit, asks for community opinion/experience/reviews, or the planner assigns
  `SourceClass::Community`.
- The first backend is Redditapis (`GET /api/reddit/search`,
  `/api/reddit/search/comments`, `/api/reddit/post/{id}/comments`).
- A thread is one `SourceRecord`; its note contains the post body and top
  comments, so the same exact-quote audit applies.
- Reddit failures disable the tool for the rest of the job and fall back to web
  search. A missing `REDDIT_API_KEY` is not a startup error.
- See `docs/reddit-search-investigation.md` for the provider comparison and
  cost model.

Credentials are declared in the root `secretspec.toml`. Nothing secret is
stored in the config file, the Nix store, or the repository. The `research`
binary, `researchd`, and `research serve` resolve `TAVILY_API_KEY` in this
order: the environment, then SecretSpec (manifest discovered upward from the
working directory, or `SECRETSPEC_FILE` / `provider.tavily.secretspec_file`).
The keyring provider is compiled in; other providers can be used by exporting
the variable or wrapping the process with `secretspec run`. The Nix devshell
ships `secretspec`, and the `just` recipes still wrap provider and worker
commands with `secretspec run` so the access reason is recorded.

---

## 7. Pipeline

1. **Intake.** Validate, create job and output directory, write `brief.md`.
2. **Plan.** LLM returns a schema-validated plan: sharpened question, 4–8 subquestions, source classes, query seeds, success criteria. Optional human review.
3. **Collect.** Per subquestion, issue 2–4 queries (one natural-language objective plus focused keywords), call the provider through the pool, extract pages, canonicalize URLs, hash content, store passages.
4. **Verify.** Deduplicate by canonical URL and content hash, group syndicated copies, score source type, and run a coverage check against the plan. Second round only for uncovered subquestions or contradictions.
5. **Synthesize.** The LLM receives only the evidence ledger, with no web tools, and returns structured claims referencing `evidence_id`s. The server renders citations; unsupported claims are dropped or marked.
6. **Audit.** Resolve every citation, verify each quote is an exact substring of stored text, report coverage and contradictions, write `audit.md`.
7. **Deliver.** Write `report.md`, `audit.md`, and `run.json` with model, provider, prompt version, git commit, timestamps, token and credit usage. Update `research/INDEX.md`.

Safety separation: planning sees no untrusted web content; synthesis sees untrusted content but has no tools. Fetched text is data, never instructions.

---

## 8. API, CLI, MCP

API under `/v1`, localhost only, bearer token generated on first run:

- `POST /v1/jobs`, `GET /v1/jobs`, `GET /v1/jobs/{id}`
- `POST /v1/jobs/{id}/cancel`, `/resume`
- `GET /v1/jobs/{id}/plan`, `POST /v1/jobs/{id}/plan/approve`
- `GET /v1/jobs/{id}/events` (SSE), `/evidence`, `/report`, `/artifacts`
- `GET /v1/providers`, `/health`
- `Idempotency-Key` on job creation; RFC 7807 errors.

CLI:

```
research run "<question>" --depth thorough [--review] [--fake]
research serve [--once] [--fake]
research job enqueue|list|show|events|cancel|resume|approve|wait|report
research provider search|extract|test
research config show
```

`researchd` is the same worker loop as `research serve` without the CLI surface.
`--fake` selects the deterministic local provider.

MCP tools mirror the service: `research_start`, `research_status`, `research_plan_get`, `research_plan_approve`, `research_cancel`, `research_evidence`, `research_report`, `research_doctor`. Resources expose `research://jobs/{id}/report`. Start with stdio; add streamable HTTP later.

---

## 9. Storage and artifacts

- SQLite in WAL mode for jobs, plans, queries, evidence index, usage, and cache metadata.
- Content-addressed blobs for raw HTTP bodies and extracted text, under the cache directory.
- Research bundle files are the human-facing, git-friendly artifact.
- `run.json` is stamped deterministically by the daemon, never by the model.

---

## 10. Nix flake

Root `flake.nix` stays small. `flake-parts` + `import-tree` imports every module under `nix/modules/`; each module owns its inputs and outputs.

- `rust.nix`: pinned toolchain via `rust-overlay`.
- `packages.nix`: `crane` builds the workspace binaries.
- `devshell.nix`: Rust, `sqlx-cli`, `just`, `cargo-nextest`, `cargo-deny`, `typos`.
- `checks.nix`: formatting, clippy, tests, `cargo-deny`, `nix flake check`.
- `nixos-researchd.nix` (phase 4): `services.researchd` with systemd, `StateDirectory`, `RuntimeDirectory`, `environmentFile`, and hardening.

Use SQLx offline metadata so builds are sandboxed. Expose `nix run .#research`, `nix develop`.

---

## 11. Testing and evaluation

- Unit: rate limiter with paused Tokio time, budget reservation, cache, URL canonicalization, dedupe, quote verification, state transitions.
- Provider: `wiremock` fixtures for Tavily search/extract, 429, 5xx, partial extract.
- Store: temporary SQLite databases and migration tests.
- API/CLI: axum tower tests, `assert_cmd`.
- MCP: `rmcp` client tests.
- End-to-end: fake provider + mock LLM produce a deterministic bundle; `insta` snapshots.
- Crash recovery: kill after N queries, restart, assert resume without duplicate spend.
- Evaluation: frozen scenario YAML; report citation-support rate, coverage, cost, p50/p95 latency.

---

## 12. Build order

| Phase | Deliverable | Exit criterion |
|---|---|---|
| 0 | Workspace, flake, config, core types | `cargo test`, `nix flake check` |
| 1 | Tavily provider, limiter, cache, budget, raw responses | Concurrency/rate-limit tests pass; `research provider search` works |
| 2 | Job service, leases, API, CLI, artifacts, fake LLM | `research run` produces a complete bundle; kill/restart resumes |
| 3 | Real LLM planner, synthesizer, auditor | Cited report with zero unresolved claims |
| 4 | MCP server, NixOS module, hardening | MCP client can start/status/report; systemd service runs |
| 5 | Evaluation harness, fallback providers | Measured citation support, coverage, cost, latency |

---

## 13. Phase 1 scope

Implemented in this phase:

- Cargo workspace and shared dependency policy.
- `research-core`: normalized search/extract types, provider enums, config.
- `research-store`: SQLite query cache and content-addressed blob store.
- `research-provider`: `SearchProvider` trait, Tavily search/extract, token-bucket rate limiter, concurrency semaphore, retries with `Retry-After`, budget reservation, cache integration, raw-response persistence.
- `research-cli`: `research provider search`, `research provider extract`, `research provider test`, `research config show`.
- `secretspec.toml` at the repository root: declares `TAVILY_API_KEY` and optional `TAVILY_TEST_QUERY`; `just` wraps provider commands with `secretspec run`.
- Tests for limiter, budget, cache, blobs, and Tavily request/response behaviour.
- Dendritic Nix flake for the workspace and dev shell.

Deferred: the job service, daemon binary, API, MCP, pipeline, LLM integration, and NixOS module. The crate boundaries and data contracts above are fixed now so those phases do not require a rewrite.

### Phase 1 verification

- `cargo fmt --all -- --check`: pass.
- `cargo clippy --workspace --all-targets -- -D warnings`: pass.
- `cargo test --workspace`: 15 tests pass (1 CLI, 1 core, 11 provider, 2 store).
- `nix build .#research`: pass; `result/bin/research --version` prints `research 0.1.0`.
- `nix flake check`: pass; `nix develop -c research --version` prints `research 0.1.0`.
- `nix develop -c secretspec --version` works; `just secrets-check` reports missing credentials until a key is configured.
- Live Tavily calls were not run because no API key was available. The provider path is covered by Wiremock tests for search, extract, retries, cache hits, budget exhaustion, and invalid requests.

The CLI surface in this phase:

```
research config show
research provider test
research provider search "<query>" [--depth ...] [--json]
research provider extract <url>... [--json]
```

---

## 14. Phase 2 status

Implemented:

- `research-core`: stable ID newtypes, `Depth`, `ReviewPolicy`, `JobBudget`,
  `ResearchRequest`, `Plan`, `Subquestion`, `QueryRecord`, `SourceRecord`,
  `Claim`, `Synthesis`, `AuditReport`, `JobStatus` with a validated transition
  graph, and `Job`.
- `research-store`: `JobStore` over SQLite with idempotent enqueue, ordered and
  targeted claiming, leases and heartbeats, cooperative cancellation, resume,
  expired lease reaping, and a `job_events` transition history. Jobs and cache
  metadata share one database, and writes are serialized inside a process to
  avoid SQLite transaction-upgrade races with the heartbeat task.
- `research-pipeline`: rule-based planner, URL canonicalization, collector with
  URL and content-hash deduplication, extractive synthesizer, exact-quote
  auditor, and the resumable bundle writer (`brief.md`, `plan.v1.*`,
  `queries.jsonl`, `sources.jsonl`, `notes/`, `progress.v1.json`, `report.md`,
  `audit.md`, `run.json`, `INDEX.md`).
- `research-service`: runner that advances the state machine through
  planning → running → synthesizing → auditing → completed, a worker loop with
  heartbeat, cancellation, wall-time enforcement, lease reaping, and a
  deterministic fake provider for credential-free tests and demos.
- `research-cli`: `research run`, `research serve`, and `research job
  enqueue|list|show|events|cancel|resume|approve|wait|report`.
- `research-daemon`: the `researchd` worker binary.
- `research-llm`: an OpenAI-compatible model client (OpenCode Go by default)
  with retries, token and cost usage, a deterministic fake, and a
  `research doctor` configuration check.
- `research-agent`: the default budgeted agent loop. The model chooses
  `search`, `extract`, `read_source`, or `finish` actions; the daemon executes
  them through the provider gateway, synthesizes from the evidence ledger, and
  audits exact quotes. Full prompts, responses, actions, and tool results are
  stored in the SQLite `agent_runs` and `agent_events` tables and exposed by
  `research job transcript`.
- `Engine::Agent` is the default for new jobs; `Engine::Deterministic` keeps
  the rule-based pipeline available as a fallback.

Remaining for Phase 2: the `/v1` HTTP API (jobs, plans, events, evidence,
report) and the MCP adapter.

### Phase 2 verification

- `just check`: formatting, clippy with `-D warnings`, and 87 tests pass,
  including deterministic and agent end-to-end runs, manual plan review, a
  kill/restart resume test that asserts completed subquestions are not
  re-queried, transcript storage, and the model-client and credential tests.
- `research run "..." --fake` produces a complete bundle whose report has
  citations and whose audit passes.
- `research serve --once --fake` and `researchd --once --fake` process queued
  jobs through the same service path.
- `result/bin/researchd --once` with `TAVILY_API_KEY` absent from the
  environment resolves the key through SecretSpec's keyring provider, so the
  packaged daemon runs without a `secretspec run` wrapper.
