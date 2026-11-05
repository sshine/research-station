# Prior art for `research-daemon`: cross-repo synthesis

**Date:** 2026-11-02
**Inputs:** `docs/surveys/laoujin-scout.md` and `docs/surveys/research-claw.md`.
**Question:** what do the two open-source projects in `external-research-agents/` offer a Rust API + CLI + MCP daemon with parallel, rate-limited search and durable research artifacts?

---

## 1. Bottom line

Neither project is a research daemon. Both are **agent orchestration shells**: a prompt pack plus a job runner around a general-purpose LLM agent, with no reusable library, no Rust, no MCP server, and no real search-provider layer. Neither supports Tavily. Both stopped active development in mid-2026.

They are still useful, but as **design quarries and data-contract references**, not dependencies:

| Repo | What it actually is | Stack | Maturity | Verdict |
|---|---|---|---|---|
| `laoujin-scout` | GitHub Issue → Claude Code → cited Jekyll post, on a self-hosted runner | Bash + Markdown prompts + GitHub Actions + Docker | 387 commits, Apr–Jul 2026, MIT, 45 bash tests (38 pass / 7 fail), no CI test job | **Mine for parts** |
| `research-claw` | Chat-first academic paper assistant: LaTeX/Overleaf, literature radar, IM bots | Python 3.11, FastAPI, 42 unpinned deps, ~41k LOC | 140 commits, Mar–Jun 2026, MIT, 52 unit tests + 34 live-LLM E2E cases, no CI | **Mine for parts** |

The important consequence: **the entire provider layer, daemon, persistence, API/CLI/MCP surface, and security model are ours to build.** There is no shortcut in either repo.

---

## 2. What neither project has

- **Tavily, Kagi, Brave, SerpAPI, or Bing.** Scout relies entirely on Anthropic `WebSearch`/`WebFetch` (opaque, non-reproducible, unrate-limited). Claw uses arXiv, OpenAlex, PubMed, DuckDuckGo, and Jina Reader; Google Scholar and Semantic Scholar are implemented but disabled; Brave is config plumbing with no implementation.
- **A provider abstraction.** Search calls are embedded in agent tools with no normalized result type, no raw-response retention, and no per-provider policy.
- **Global rate limiting.** Claw has one good local pattern: arXiv uses a shared client, 3 s delay, semaphore(2), and 429 backoff. Everything else is semaphore(2) at best, and DuckDuckGo has nothing. Scout has none.
- **A search cache.** Neither caches search results. Scout caches nothing; Claw has no search cache.
- **A daemon-grade job model.** Scout uses one GitHub Actions job per run (270-minute timeout, no queue, no cancellation, no retry, no budget). Claw has a real DAG and resume state machine, but in-process, unbounded, and without leases or heartbeats.
- **MCP.** Neither exposes one.
- **A real database.** Claw has an unused SQLite skeleton; everything else is JSON/JSONL/Markdown. Scout is files and git worktrees.
- **SSRF protection, prompt-injection boundaries, HTTP auth, or secret management.** Both are explicit about running with broad permissions and trusting fetched content. Claw serves arbitrary absolute host paths and runs `bash` with `shell=True`; Scout runs Claude with `--dangerously-skip-permissions` and keeps a readable SSH key in the worker.
- **Claim-level citation verification.** Both produce citation ledgers, but the model writes them. Validation checks shape, not truth. Neither verifies a quote against stored page text.

---

## 3. What is worth borrowing

### 3.1 From `laoujin-scout`

| Concern | Artifact | What to port |
|---|---|---|
| Depth tiers and budgets | `README.md:125-131`, `scripts/lib-models.sh:17-35`, `skills/scout-research/SKILL.md:90-97` | `recon` / `survey` / `expedition` as a Rust enum mapping to query budget, researcher count, citation target, review depth, and model tier. |
| Query sharpening and decomposition | `skills/scout-research/sharpen.md:38-107` | Typed `QueryPlan` with `subquestions`, `constraints`, `coverage_suggestions`, `depth`, and provenance (`raw_topic` vs `sharpened_topic`). Preserve steering hints verbatim. 2–8 sub-topics. |
| Evidence ledger | `skills/scout-research/SKILL.md:137-141`, `scripts/merge_ledgers.sh:19-30`, `scripts/validate_ledger.sh:17-71` | JSONL fields `n`, `url`, `claim`, `source_type`, `quote`, plus first-seen URL dedupe, stable renumbering, and "every marker resolves". Extend with `content_hash`, `fetched_at`, `http_status`, `retrieval_method`, and server-side quote verification. |
| Evidence-first output contract | `SKILL.md:101-146`, `CLAUDE.md:9-20` | TL;DR first, inline citations, no trailing reference list, **no URL ⇒ no claim**, source-type labels. Make it a renderer plus validator. |
| Reviewer deltas | `.claude/agents/scout-reviewer.md:19-56` | Blocking categories: citation–claim mismatch, orphan claim, broken numbering, empty ledger fields. Implement the deterministic subset; use an LLM only for claim/quote entailment. |
| Status taxonomy | `scripts/run-decompose.sh:138-175,242-309` | `success`, `error_with_content`, `failed`, `skipped_success`, `skipped_soft_timeout`, `failed_hard_timeout`. Persist per-child status and skip successful children on rerun. |
| Deterministic metadata | `scripts/run.sh:242-258`, `scripts/inject_cost.sh`, `scripts/inject-run-metadata.sh` | Never trust the agent to report cost, duration, or model. Capture from the execution result and stamp it after the fact. |
| Artifact health scan | `skills/scout-triage/scan.py:1-80` | `doctor` command categories: `DEAD`, `GENUINE_FAILURE`, `FALSE_FLAG`, `MANIFEST_MISMATCH`, `LEDGER_MISMATCH`, `MISSING_*`. |
| Human gate | `scripts/issue-comment.sh:60-95`, `.github/workflows/research.yml:161-172` | Plan review before spending budget. In the daemon, expose it as a `plan.review` API/CLI step. |

Scout's strongest single idea is the **evidence-first output contract combined with a ledger that every citation marker must resolve**. Its weakest is that the ledger is agent-authored and only shape-checked.

### 3.2 From `research-claw`

| Concern | Artifact | What to port |
|---|---|---|
| Task DAG and state machine | `agent/scheduler/schema.py:21-91` | Typed statuses, dependencies, acceptance spec, assigned agent, output dir, artifacts, iteration budget, reviewer feedback, retry count. Extend with `query_plan`, `evidence_ids`, `provider`, `budget`, `deadline`, `lease`. |
| Ready-set batch runner | `agent/scheduler/batch_runner.py:52-113` | Small "run every task whose dependencies are complete, then return control" loop. Add a global semaphore, per-provider limits, and a batch cap. |
| Resume semantics | `agent/task_agent.py:165-281` | Convert stale in-flight states to `INTERRUPTED` on load; expose recoverable tasks; reset a failed task and its unfinished descendants; require `--force` if completed descendants would be reset. |
| Append-only filesystem ledger | `core/memory/store.py:30-74,162-209,405-565,646-727` | One JSONL index record per entry plus one Markdown body; `kind`, `scope`, `intent`, `title`, `summary`, `tags`, `source`, `ttl`, `parent_id`; atomic temp-file replace and `flock`; cursor pagination; TTL and lazy GC. Replace prose `content` with typed evidence records. |
| Job/state/run separation | `core/automation/store_fs.py:24-61`, `core/automation/models.py:61-148`, `core/automation/runtime.py:329-375` | Separate job definitions, job state, and run records. `JobRun` fields: trigger, status, timestamps, output excerpt, error, token usage. |
| Cron policy | `core/automation/scheduler_aps.py:88-106` | Coalesce missed runs, one instance per job, one-hour misfire grace, heartbeat. Copy the policy, not the Python. |
| Per-provider pacing | `agent/tools/arxiv_search.py:8-13,150-206` | Shared client, persistent delay, semaphore, 429 backoff. Generalize into a Rust `RateLimiter` trait: token bucket + concurrency semaphore + retry policy + circuit breaker. |
| Research prompt assets | `config/prompts/skill_radar.txt:50-77`, `config/.skills/paper-research-form/`, `config/.skills/ml-paper-writing/references/citation-workflow.md` | Multi-round querying, source priority, query patterns, metadata verification, normalized-title dedupe, published-over-preprint preference, "verify in 2+ sources", date cutoffs. Copy as data with attribution. |
| Scenario tests | `tests/cases/*.yaml`, `tests/scripts/run_batch.py:52-99`, `tests/scripts/judge_eval.py` | Case declares project, mode, query, timeout, expected outcomes; runner archives the workspace and optionally LLM-grades the trace. Keep the shape; prefer deterministic assertions. |

Claw's strongest single idea is the **typed DAG with dependency-aware ready-set execution and explicit resume semantics**. Its weakest is that the execution is unbounded and the review gate is prose-only.

---

## 4. Anti-patterns to design out

Both repos converge on the same failure modes. These are requirements for `research-daemon`, not just warnings:

1. **Do not let the LLM be the search transport.** The daemon owns provider calls, normalizes results, records raw responses, and enforces budgets.
2. **Do not trust agent-written evidence.** Fetch and extract server-side, hash the content, store the passage, and verify quoted spans against stored text.
3. **Do not use files as the job database.** Use a real store for jobs, attempts, leases, evidence, and plans. Write immutable artifacts alongside it.
4. **Do not allow unbounded fan-out.** Every fan-out goes through a global job semaphore and per-provider limiter. Claw's `asyncio.gather` over all ready tasks is the pattern to replace.
5. **Do not let a plan be overwritten.** Claw's `latest_plan.json` is overwritten per run. Plans must be immutable and versioned by run ID.
6. **Do not ship a permissive fetch/security model.** SSRF allowlists, redirect and body-size caps, no shell, authenticated API, secret store, and a clear untrusted-content boundary around fetched text.
7. **Do not build three surfaces separately.** One service core, with API, CLI, and MCP as thin adapters. Claw's ~60 FastAPI routes and 2,165-line gateway module are the warning.
8. **Do not inherit dependency weight.** Claw has 42 unpinned Python dependencies including browser automation, pandas, Docker, and IM SDKs. Keep the Rust workspace small and auditable.

---

## 5. Implications for the architecture (constraints, not a plan)

- **Provider layer is the first real component.** A `SearchProvider` trait with Tavily first, normalized results, raw-response retention, and a `RateLimiter` (token bucket + semaphore + retry/backoff + circuit breaker + cache). This is the part both repos lack and the part the user prioritised.
- **The daemon owns fetch and extraction.** The agent proposes queries and synthesises; it does not fetch arbitrary URLs or write the evidence ledger directly.
- **The job model needs:** immutable plan versions, a DAG of sub-questions, typed statuses, leases and heartbeats, cancellation, per-job budget and deadline, idempotency keys, and resume.
- **The artifact model needs:** `plan.json` per version, `queries.jsonl`, `sources.jsonl` (evidence + hashes + passages), `notes/`, `report.md`, `audit.md`, plus a run record. Store metadata in SQLite; write artifacts as files for human inspection and git.
- **The API/CLI/MCP split should be thin.** One `Service` crate; `api`, `cli`, and `mcp` crates translate transport into service calls. MCP tools should mirror the service operations (`research.start`, `research.status`, `research.plan.review`, `research.evidence`, `research.report`, `research.cancel`), not reimplement the pipeline.
- **Evaluation is part of the daemon, not an afterthought.** The scenario-case format from Claw and the search-evaluation contract from Parallel are both compatible; pick one and freeze it before tuning prompts.

---

## 6. Open questions for the architecture phase

1. **Tavily surface.** Which Tavily endpoints are in scope: `/search`, `/extract`, `/crawl`, `/map`? Does the free tier cover the expected query volume, and what is the per-key concurrency and rate limit we must model?
2. **Store.** SQLite first, or Postgres from day one? The evidence ledger wants transactional writes; the artifacts want plain files.
3. **Deployment.** Does the Nix flake produce a systemd service, a container, or both? Does the daemon need multi-user tenancy or only a single operator?
4. **MCP transport.** stdio only, or also streamable HTTP? Does it share the daemon or launch its own process?
5. **Human gate.** Is plan review mandatory by default, or only above a cost threshold?
6. **Rust async runtime and HTTP stack.** Tokio + axum + reqwest + sqlx is the obvious default; confirm before scaffolding.
7. **LLM access.** Which provider/model drives query planning and synthesis, and is it called by the daemon or injected by the client?

---

## 7. References

- `docs/surveys/laoujin-scout.md` (41 KB, line-cited)
- `docs/surveys/research-claw.md` (51 KB, line-cited)
- Upstream: `git@github.com:Laoujin/Scout.git` at `4bf889d`; `git@github.com:nanoAgentTeam/research-claw.git` at `ce3a4ac`.
