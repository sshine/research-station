# research-daemon

Rust workspace for the research-station daemon, API, CLI, and MCP surfaces.

Phase 1 is implemented: the Tavily provider layer, rate limiting, budgets,
caching, raw-response storage, and a CLI to exercise them.

Phase 2 is implemented through the working pipeline and daemon: durable jobs,
leases, crash recovery, a deterministic planner/collector/synthesizer/auditor,
and a resumable artifact bundle. `research run` completes a job in-process;
`research serve` and `researchd` process the queue. The HTTP API and MCP server
are the remaining Phase 2 surfaces.

See [`../architecture.md`](../architecture.md) for the full design.

---

## Layout

```
crates/
  research-core/      domain types, config, errors; no I/O
  research-store/     SQLite query cache, blob store, and durable job queue
  research-provider/  SearchProvider trait, Tavily, fake provider, limiter, budget
  research-pipeline/  planner, collector, synthesis, audit, bundle writer
  research-service/   runner, worker loop, provider construction
  research-daemon/    `researchd` binary
  research-cli/       `research` binary
```

## Build and test

```bash
cd research-daemon
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Or from the repository root:

```bash
just check
```

With Nix:

```bash
nix develop
nix build .#research        # result/bin/research and result/bin/researchd
nix flake check
nix run .#research -- run "How do Tavily and Kagi compare?" --fake
```

## Configuration

Copy the example config:

```bash
cp research-daemon/config/researchd.example.toml research-daemon/config/researchd.toml
```

The default search locations are:

1. `--config <path>`
2. `$RESEARCHD_CONFIG`
3. `research-daemon/config/researchd.toml`
4. `$XDG_CONFIG_HOME/research-station/researchd.toml`
5. built-in defaults

The API key is never stored in the config file. It is declared in
`secretspec.toml` at the repository root and resolved by [secretspec], so each
developer chooses where the value lives and nothing secret is committed.

Run these from the repository root:

```bash
secretspec config global init      # once: choose a provider, e.g. the system keyring
just secrets-set                   # prompts for TAVILY_API_KEY
just secrets-check                 # report what is still missing
just provider test                 # one billed smoke search
```

`secretspec run` injects the variable into the child process. The daemon and
CLI also resolve the key themselves when it is absent from the environment:
they load `secretspec.toml` and ask the configured provider (the system keyring
by default), so `./result/bin/researchd` works without a wrapper. The keyring
provider is compiled into the binary; for other SecretSpec providers, either
export `TAVILY_API_KEY` or keep using `secretspec run`.

Direct binaries find the manifest by searching upward from the working
directory. Set `SECRETSPEC_FILE=/path/to/secretspec.toml` or
`provider.tavily.secretspec_file` in `researchd.toml` when running from
elsewhere, for example under systemd. On Linux the keyring provider needs
gnome-keyring or KWallet running.

Set `TAVILY_TEST_QUERY` to change the query `provider test` uses.

The SQLite database and blob store default to `$XDG_CACHE_HOME/research-station/`.
Jobs and cache metadata share the database; `store.database_path` selects it,
and the older `store.cache_path` remains a fallback.

[secretspec]: https://secretspec.dev

## CLI

The `just` recipes below additionally wrap commands with secretspec so the
access reason is recorded. The raw `research` binary resolves the key itself
when it is not already in the environment.

```bash
# Show resolved configuration without secrets.
research config show

# Verify provider, key, cache, and blob store.
just provider test

# Search.
just search "Tavily vs Kagi for research agents" \
  --depth advanced \
  --max-results 10 \
  --raw-content

# Search with freshness and source constraints.
just search "AI search API pricing" \
  --topic news \
  --time-range month \
  --include-domain tavily.com \
  --include-domain kagi.com \
  --json

# Extract up to 20 URLs.
just extract https://example.com/article --depth advanced
```

`--json` prints the full normalized `ProviderResponse`, including `cache_hit`,
`raw_hash`, `credits`, `request_id`, and `latency_ms`.

## Durable jobs

Phase 2 stores jobs in the same SQLite database as the query cache. A worker
claims the oldest queued job under a lease, heartbeats the lease, and advances
the job through a validated state machine. If the worker dies, the expired
lease is reaped into `interrupted`, and `resume` puts the job back on the queue
without losing its attempt history. Cancellation requested before a crash is
honoured by the reaper instead of being forgotten.

```bash
# Add a job; the output directory defaults to research/<date>-<slug>.
research job enqueue "How do Tavily and Kagi compare?" --depth thorough

# A repeated idempotency key returns the original job.
research job enqueue "ignored" --idempotency-key weekly-pricing

# Inspect.
research job list [--status queued] [--limit 20]
research job show <id>
research job events <id>

# Control.
research job cancel <id>
research job resume <id>

# A manual-review job pauses after planning.
research job approve <id>

# Wait for a job, then print its report.
research job wait <id>
research job report <id>
```

`research job claim` and `research job reap` are hidden development commands for
exercising a worker without the daemon. The worker calls the same `JobStore`
methods.

## Pipeline and daemon

`research run` enqueues the question, claims that job, and processes it in the
current process. `research serve` (or `researchd`) polls the queue and processes
jobs until interrupted. Both accept `--fake`, which uses the deterministic local
provider and needs no API key:

```bash
# From the repository root, without credentials:
just demo "How do Tavily and Kagi compare?"

# Real Tavily run:
just research "How do Tavily and Kagi compare?" --depth thorough

# Process the queue with the real provider, once:
just worker --once

# Long-lived worker with no credentials (deterministic fake provider):
just worker-fake

# One question, end to end. Remove --fake to use Tavily.
research run "How do Tavily and Kagi compare?" --depth thorough --fake

# Process the current queue once.
research serve --once --fake

# Run the long-lived worker.
researchd --fake
# or
research serve --fake
```

The pipeline is deterministic in Phase 2:

1. **Plan** — a rule-based planner writes `plan.v1.json` and `plan.v1.md`.
2. **Collect** — two query seeds per subquestion call the provider, deduplicate
   URLs and content hashes, and write `queries.jsonl`, `sources.jsonl`, and
   `notes/<source-id>.md`.
3. **Synthesize** — exact sentences from stored notes become claims; no model
   invents prose.
4. **Audit** — every quote must be an exact substring of its stored note.
5. **Deliver** — `report.md`, `audit.md`, `run.json`, and an `INDEX.md` row.

A job directory contains:

```
brief.md          original request and budgets
plan.v1.json/md   immutable, versioned plan
queries.jsonl     every provider call and its cost
sources.jsonl     accepted sources and quality metadata
notes/            exact stored text, one file per source
progress.v1.json  completed subquestions and plan approval
report.md         cited report
audit.md          quote verification and coverage
run.json          provider, versions, timestamps, usage, audit result
```

Progress is persisted after each subquestion. If the worker dies, the expired
lease becomes `interrupted`; `research job resume <id>` requeues it, and the
next worker skips completed subquestions instead of paying for them again.

## Provider behaviour

- **Rate limiting:** token bucket (`requests_per_minute`, `burst`) plus a
  concurrency semaphore (`max_concurrency`).
- **Retries:** 429, 5xx, and capacity-exceeded responses retry with exponential
  backoff and jitter; `Retry-After` is honoured.
- **Quota errors:** Tavily `432`/`433` become `QuotaExceeded`; `434` becomes
  `CapacityExceeded`.
- **Cache:** successful responses are cached by a hash of provider, operation,
  and normalized request. Cache hits cost no credits and do not consume the
  query budget.
- **Budget:** query slots are reserved before a request; failed sends refund the
  slot. Credits are recorded from Tavily's `usage.credits`, falling back to the
  documented estimate.
- **Raw responses:** every successful HTTP body is stored in a content-addressed
  blob store and its hash is returned in the response.

## Notes

- Tavily's `/search` accepts `max_results` 1–20; `/extract` accepts 1–20 URLs.
- `advanced` search costs 2 credits; `basic`, `fast`, and `ultra-fast` cost 1.
- `advanced` extract costs 2 credits per 5 successful URLs; `basic` costs 1.
- Set `cache_ttl_seconds = 0` in the config to effectively disable caching.
