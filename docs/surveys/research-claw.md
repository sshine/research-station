# Survey: `research-claw`

**Repository:** `external-research-agents/research-claw` (remote: `git@github.com:nanoAgentTeam/research-claw.git`)
**Surveyed:** 2026-11-02, from a clean working tree at `ce3a4acd65d81f5f7681a90df0bf4abab288cc7d`.
**Question:** what does this repository offer as prior art for `research-daemon`, a Rust workspace with API + CLI + MCP, parallel rate-limited search, a query plan and evidence ledger, and durable artifacts?

---

## TL;DR

`research-claw` is **not a research daemon**. It is a self-hosted, chat-first academic paper assistant: LaTeX/Overleaf project management, literature search agents, IM channels, a cron-driven "research radar", and a `/task` mode that decomposes a goal into a DAG and runs sub-agents. The daemon-like pieces are a single FastAPI gateway process, APScheduler, and an in-process task scheduler.

It has three genuinely useful pieces of prior art:

1. **A task DAG and resume state machine** (`agent/scheduler/schema.py`, `agent/task_agent.py`, `agent/scheduler/batch_runner.py`) that is close to the job/plan model `research-daemon` needs.
2. **A filesystem-backed, append-only knowledge/memory ledger** (`core/memory/store.py`) with scope/intent/kind metadata, TTL/GC, atomic writes, and JSONL indexing — a good shape for an evidence ledger, though it is not an evidence ledger today.
3. **Prompt-level research discipline** (`config/prompts/skill_radar.txt`, `config/.skills/paper-research-form/`, `config/.skills/ml-paper-writing/references/citation-workflow.md`) covering multi-round querying, source priority, metadata verification, deduplication, and date cutoffs.

What it does **not** have: Tavily, Kagi, or any real web-search provider beyond DuckDuckGo; a global rate limiter or search cache; MCP; HTTP auth; SSRF protection; prompt-injection defenses; a database; a durable job queue; a query-level plan; or claim-level citations.

**Verdict:** **mine for parts** — reuse the data-model and operational ideas, copy the prompt/skill assets, reimplement everything in Rust. Do not adopt the Python application or its search/safety stack.

---

## 1. Purpose and problem; target user

The README describes "Your self-hosted AI research assistant" that lets a researcher "Manage papers · search literature · track deadlines · collaborate across channels" (`README.md:9-11`). The concrete product is an AI agent for **academic paper writing**: it creates LaTeX projects, syncs them to Overleaf, compiles PDFs, searches literature, tracks conference deadlines, and answers on CLI/Web UI/Feishu/Telegram/QQ/DingTalk (`README.md:55-68`, `README.md:99-139`). The overview guide is explicit: "Research Claw is an AI Agent system designed for academic paper writing" (`README/guide/01_Overview_en.md:5`).

Target user: an individual academic researcher or student who wants a local, self-hosted assistant for LaTeX/Overleaf work and literature monitoring, is comfortable with Python/venv, and prefers Chinese-first UI and docs (`settings.default.json:61-64`; `README.md:27`). It is not aimed at programmatic consumers, teams, or a service that other software calls.

The problem it solves is "one assistant instead of switching between editor, Overleaf, terminal, and search engine" (`README.md:57`). The research-radar feature adds scheduled literature monitoring and push notifications (`README.md:406-425`). That is adjacent to, but narrower than, general-purpose research jobs.

---

## 2. Language, stack, build system, dependency weight, install/run

| Item | Finding |
|---|---|
| Language | Python only; `requires-python = ">=3.11"` (`pyproject.toml:5`). No Rust/Node/Go components. |
| Build | setuptools via `pyproject.toml`; packages are found by prefix (`pyproject.toml:7-12`). There is **no** `[project.scripts]` entry point, so the `open_research_claw` command referenced by the CLI’s process matcher is not installed by this file (`pyproject.toml:1-12`, `cli/main.py:93-106`). |
| Dependency management | `pip install -r requirements.txt` (`README.md:155-162`). 42 direct requirements, all lower-bound unpinned (`requirements.txt:1-42`). No lockfile, no hashes, no constraints file. |
| Heavy dependencies | `browser-use`, `MarkItDown[all]`, `litellm`, `pandas`, `pymupdf`/`pymupdf4llm`, `docker`, `python-telegram-bot`, `lark-oapi`, `overleaf-sync`, `pyoverleaf`, `apprise`, `dingtalk-stream`, `scholarly`, `semanticscholar`, `arxiv` (`requirements.txt:5-42`). |
| Runtime assumptions | Linux/macOS; Windows only via WSL2 because it relies on POSIX signals/process management (`README.md:164-224`). |
| Install/run story | Clone, venv, `pip install -r requirements.txt`, then `python cli/main.py gateway --port 18790` or `python cli/main.py agent` (`README.md:151-162`, `README.md:226-277`). No Dockerfile, no Nix expression, no packaged wheel, no systemd unit. |
| Repo size | 47 MB working tree (excluding `.git`), 98 MB `.git`, 533 tracked files, 40,926 Python LOC. Largest files: `agent/services/gateway_server.py` (2,165), `agent/loop.py` (1,688), `agent/services/commands.py` (1,358), `core/project.py` (1,274), `agent/tools/task_tools.py` (1,175), `cli/main.py` (1,061). |

The install story is a developer checkout, not a deployable service. It also copies `settings.default.json` to `settings.json` on first gateway start (`cli/main.py:783-790`).

---

## 3. Architecture

### 3.1 Components

The README’s own diagram (`README.md:283-305`) is accurate at a high level:

- **Channels:** CLI, FastAPI Web UI + WebSocket, Feishu, Telegram, QQ, DingTalk (`README.md:285-299`; `channels/`).
- **MessageBus:** in-memory `asyncio.Queue` decoupling channels from the agent (`bus/queue.py:11-54`).
- **AgentLoop / AgentEngine:** the main ReAct loop, tool dispatch, context management, history/trace (`agent/loop.py`, `core/llm/engine.py`).
- **ToolRegistry / ToolLoader:** class-path-driven tool loading from `config/tools.json`, filtered by per-role profiles in `config/agent_profiles/*.json` (`agent/tools/loader.py:23-118`; `config/tools.json:1-250`).
- **Scheduler/sub-agents:** `PlannerAgent`, `SchedulerEngine`, `BatchRunner`, `SDDExecutor`, and the `TaskGraph` schema (`agent/scheduler/`).
- **Automation runtime:** APScheduler cron jobs per project, file-backed job definitions and state, push targets (`core/automation/`).
- **Memory/profile:** filesystem project memory and research-profile builders (`core/memory/`, `core/profile/`).

### 3.2 Process model

There is **one long-lived process** in gateway mode: `agent/services/gateway_server.py` constructs the FastAPI app, a global `MessageBus`, the `AgentLoop`, the IM runtime, and `AutomationRuntime` at startup (`agent/services/gateway_server.py:28-99`, `196-229`). All of these share one asyncio event loop. CLI agent mode is a separate short-lived process (`cli/main.py:353-677`), and `subagent` can launch a standalone process (`cli/main.py:678-688`).

Normal sub-agents are **in-process** `AgentLoop` objects, not worker processes (`agent/tools/swarm.py:129-182`). There is no daemon supervisor, no IPC protocol for jobs, and no broker. The only IPC-like primitive is `core/infra/protocol.py`, a length-prefixed JSON framing helper (not used by the gateway for jobs).

### 3.3 Job model

Two distinct models coexist:

**Automation jobs** — `AutomationJob` is a cron + prompt record with `id`, `name`, `type` (`normal` or `task`), `JobSchedule`, `enabled`, `managed_by`, `frozen`, `OutputPolicy`, and free-form metadata (`core/automation/models.py:61-116`). `JobRun` captures `run_id`, `project_id`, `job_id`, `trigger`, timestamps, `status` (`running|success|failed|skipped`), output excerpt, error, and metadata (`core/automation/models.py:119-148`).

**Research/task jobs** — `TaskGraph` contains `ResearchTask` records with `id`, `title`, `description`, `type`, `dependencies`, `spec` (reviewer acceptance criteria), `assigned_agent`, `output_dir`, `status` (`pending|running|reviewing|completed|failed|blocked|interrupted`), `artifacts`, `max_iterations`, `feedback_history`, and `retry_count` (`agent/scheduler/schema.py:5-49`). `TaskSession` persists the goal, phase, proposal, deliverables, graph, artifacts, committed flag, round id, and auto-mode flag (`agent/task_agent.py:22-42`).

This is the closest thing in the repo to a **query plan + job record**, but it is a general work DAG, not a search-query plan. There is no query/evidence entity.

### 3.4 Scheduling and concurrency

- Cron: APScheduler `AsyncIOScheduler`, `CronTrigger.from_crontab`, `coalesce=True`, `max_instances=1`, `misfire_grace_time=3600` (`core/automation/scheduler_aps.py:88-106`). A heartbeat logs scheduler health every 30 minutes (`core/automation/scheduler_aps.py:30-31`, `164-184`).
- Task DAG: `SchedulerEngine.run_tick()` finds `PENDING` tasks whose dependencies are `COMPLETED` and launches each with `asyncio.create_task` (`agent/scheduler/engine.py:25-56`). The concurrency limit is explicitly **not implemented**: `# Check concurrency limit if needed (omitted for now)` (`agent/scheduler/engine.py:49`). The loop polls every second (`agent/scheduler/engine.py:152`).
- Batch mode: `BatchRunner.run_next_batch()` gets all currently ready tasks and runs them via `asyncio.gather` (`agent/scheduler/batch_runner.py:52-100`). There is no batch-size or concurrency cap; a wide DAG fans out fully.
- Interactive `/task` drives `BatchRunner` in a loop until all tasks complete or no ready tasks remain (`agent/tools/task_tools.py:799-851`).
- Tool calls within one LLM turn are executed with `asyncio.gather` (`core/llm/engine.py:435-438`; `agent/loop.py:1147-1150`). Per-provider semaphores below limit actual fan-out.
- Messages are queued only in memory; restart loses queued work (`bus/queue.py:19-24`).

The scheduler is a DAG evaluator, not a durable work queue. There is no lease, retry queue, dead-letter queue, or crash recovery for in-flight automation runs.

---

## 4. Research pipeline

### 4.1 Two pipelines

**Interactive `/task` (five phases).** `TaskPhase` is `UNDERSTAND → PROPOSE → PLAN → EXECUTE → FINALIZE` (`agent/task_agent.py:22-27`). The user reviews a proposal and a DAG, then `/start` runs it. Phase prompts are in `config/prompts/task_phase_*.txt`; e.g. UNDERSTAND says explore with `ls`/`read_file`/`bash` then call `task_propose` (`config/prompts/task_phase_understand.txt:8-9`), and FINALIZE says merge worker outputs into clean deliverables and never invent numbers (`config/prompts/task_phase_finalize.txt:8-24`).

**Scheduled radar.** Seven default jobs are installed per project: daily scan, weekly digest, urgent alert, direction drift, profile refresh, conference track, deadline watch (`core/automation/radar_defaults.py:40-296`). Each is a prompt executed by an automation-session agent, with project memory and recent run history injected (`core/automation/executor.py:86-127`). The radar skill enforces project navigation, context extraction, job-state cutoff, multi-source search, and state persistence (`config/prompts/skill_radar.txt:1-77`).

There is no shared, typed "research pipeline" object. The pipeline is prose plus tool calls; behavior varies with the LLM.

### 4.2 Query generation and decomposition

- `PlannerAgent.create_plan()` asks an LLM for a JSON task graph, strips markdown fences, parses with `json.loads`, builds `TaskGraph`, writes `latest_plan.json` and a Mermaid `latest_plan.md` (`agent/scheduler/planner.py:31-138`). The plan is overwritten each time; there is no plan history.
- `/task` builds the graph from a proposal with retries on JSON parse and cycle errors (`agent/tools/task_tools.py:417-518`). Cycle detection is DFS (`agent/task_agent.py:113-136`).
- There is **no query-level planner**: no list of queries, no per-query provider assignment, no expected coverage, no budget per query, no query result state. Queries are invented by the agent at execution time from prompts.
- Radar prompts do impose a query method: read the profile, use the last run as `date_from`, search arXiv/PubMed/OpenAlex, deduplicate by paper ID, and push findings (`core/automation/radar_defaults.py:48-75`). The radar skill adds three rounds (relevance, recency, citation depth), requires at least three distinct queries across at least two tools, and forbids near-duplicate queries (`config/prompts/skill_radar.txt:72-77`).
- `paper-research-form` provides source priority and query patterns: Semantic Scholar → arXiv → ACL Anthology → OpenReview, with `site:` query templates and "use a small number of focused queries instead of one broad, noisy query" (`config/.skills/paper-research-form/references/search-strategy.md:21-54`).

### 4.3 Source selection

Source selection is tool availability plus prompt guidance. The automation profile exposes `arxiv_search`, `openalex_search`, `pubmed_search`, `web_search`, and `web_fetch` (`config/agent_profiles/automation_agent.json:4-26`). The task-worker profile exposes a smaller set (`config/agent_profiles/sdd_worker.json`). Radar prompts prefer arXiv for CS/AI preprints, PubMed for biomedicine, OpenAlex for cross-domain coverage (`core/automation/radar_defaults.py:55-59`). The paper-survey skill prefers published venue metadata over preprints and gives a canonical-link priority order (`config/.skills/paper-research-form/references/csv-schema.md:81-88`).

No source-quality scoring, domain allowlist, or per-source trust model exists. `web_search` returns DuckDuckGo results with no domain filtering.

### 4.4 Fetching and extraction

`web_fetch` (`agent/tools/web_reader.py`) does:

- `requests.get(url, timeout=30)` with no scheme/host validation (`agent/tools/web_reader.py:71-77`).
- PDFs: cache the original under `~/.open_research_claw/cache/papers/`, convert with `pymupdf4llm.to_markdown`, return the first 10 pages as a preview (`agent/tools/web_reader.py:14-16`, `113-158`).
- Downloadable text/binary types: cache under `~/.open_research_claw/cache/common/`, inline text up to 100k chars (`agent/tools/web_reader.py:19-26`, `173-216`).
- HTML: fetch through Jina Reader (`https://r.jina.ai/<url>`), return up to 100k chars (`agent/tools/web_reader.py:92-108`).

There is no readability extraction, no boilerplate removal beyond Jina, no per-domain fetch policy, and no robots/ToS handling.

### 4.5 Synthesis

Workers write files into isolated overlay directories. A worker/reviewer loop runs up to `max_retries = 3`; the reviewer is an LLM that must end with `CONCLUSION: PASS` or `CONCLUSION: FAIL: <reason>` (`agent/scheduler/executor.py:67-97`, `493-590`). On pass, the overlay diff is copied to `core/_task_workers/{task_id}_r{round}/` (`agent/scheduler/executor.py:386-416`). The main agent then merges those outputs into the project root during FINALIZE, optionally compiles/fixes LaTeX, generates a commit message with an LLM, commits, and **deletes** `_task_workers/` (`agent/tools/task_tools.py:918-999`).

Synthesis is therefore LLM-driven file merging, not a structured report synthesizer. There is no report schema, no section graph, no contradiction pass, and no evidence table. The paper-survey skill is the exception: it defines an 8-column CSV output (`title`, `abstract_snippet`, `summary`, `relevance`, `authors`, `year`, `venue`, `link`) with deterministic ordering and dedupe rules (`config/.skills/paper-research-form/references/csv-schema.md:5-115`, `merge-and-delivery.md:33-57`).

### 4.6 Citations

Citation handling is minimal and mostly prompt-level:

- The engine accumulates `web_search` result dicts that have an `href` and appends a markdown "参考来源" list to the final assistant message (`core/llm/engine.py:313-322`, `462-471`). This only covers `web_search`, not arXiv/OpenAlex/PubMed, and it is a flat list, not claim-linked.
- OpenAlex results include generated BibTeX entries (`agent/tools/openalex_search.py:278-337`). arXiv/PubMed return metadata and URLs (`agent/tools/arxiv_search.py:226-255`, `agent/tools/pubmed_search.py:212-232`).
- The `citation-workflow.md` skill documents a five-step "search → verify in 2+ sources → retrieve BibTeX → validate claim → add" process and warns about hallucinated citations (`config/.skills/ml-paper-writing/references/citation-workflow.md:39-85`). It is **not wired into the runtime**; it is a document the agent may activate.
- Project prompts require BibTeX entries to come from a real search tool result (`config/prompts/project_active.txt:28`) and require PDF fetch success to be checked before treating content as real (`config/prompts/project_active.txt:63-67`). These are instructions, not enforced invariants.

No evidence ledger, no claim-to-source mapping, no citation audit, no source snapshot, and no stable citation IDs.

### 4.7 Report structure

There is no fixed report schema. Scheduled radar jobs specify their own output format in prose: daily scan requires title, full clickable URL, authors, 3–5 sentence summary, relation label, and project implication per paper (`core/automation/radar_defaults.py:63-75`); weekly digest has a five-part outline (`core/automation/radar_defaults.py:99-110`). `/task` FINALIZE asks for clean reader-facing files such as `report.md` or `paper.tex` (`config/prompts/task_phase_finalize.txt:11-24`). The paper-survey skill produces a CSV plus a markdown summary (`config/.skills/paper-research-form/SKILL.md:71-95`).

### 4.8 Stopping criteria

- LLM iteration budget: default `maxIterations: 100` (`settings.default.json:34-38`), read into `AgentLoop` (`agent/loop.py:134`).
- Worker budget: at least 60 iterations, configurable per task; timeout is `max(max_worker_iters * 15, 900)` seconds (`agent/scheduler/executor.py:251-264`). The timeout is checked as events stream (`agent/scheduler/executor.py:273-278`).
- Reviewer budget: 20 iterations (`agent/scheduler/executor.py:539`).
- Loop detection: after repeated identical tool calls, inject a warning; after three consecutive deadlocks, run a separate LLM "meta-diagnosis" and reset context (`core/llm/engine.py:407-432`, `agent/loop.py:1080-1125`).
- DAG completion: all tasks `COMPLETED`; if no task is running but pending tasks remain, mark them failed and abort (`agent/scheduler/engine.py:123-150`). Batch mode breaks when `all_complete`, or when no task ran in a batch (`agent/tools/task_tools.py:837-851`).
- Scheduled jobs: one run per cron fire, `max_instances=1`, coalesced (`core/automation/scheduler_aps.py:102-106`).

There is **no coverage/saturation criterion** ("enough sources found"), no per-query budget, no token/cost ceiling at the job level, and no time budget for a whole research job except the worker timeout.

---

## 5. Search providers, rate limiting, retries, caching, parallel querying

### 5.1 Providers

| Provider | Status | Evidence |
|---|---|---|
| **Tavily** | **Not supported.** A repository-wide search for `tavily` found no code or config. | `grep -rniE "tavily"` over `*.py`, `*.json`, `*.md`, `*.toml` returned no matches. |
| **Kagi** | **Not supported.** Same search for `kagi` returned no matches. | As above. |
| **Brave** | **Configured but dead.** `WebSearchConfig.api_key` is commented "Brave Search API key" (`config/schema.py:105-108`), `cli/main.py` passes it into `AutomationRuntime` (`cli/main.py:433`), and it is stored on `AgentLoop` (`agent/loop.py:82`, `135`), but no search tool reads it. The only `web_search` implementation is DuckDuckGo. | `core/tools/web_search.py:98-106`; grep for `brave_api_key` shows only plumbing, no consumer. |
| **DuckDuckGo** | The only `web_search` provider. Uses `ddgs`/`duckduckgo_search`, no API key. | `core/tools/web_search.py:48-78`, `98-106`; `requirements.txt:5`. |
| **arXiv** | Supported, with field-prefix queries, date/category filters, sorting, pagination. | `agent/tools/arxiv_search.py:16-257`; enabled in `config/tools.json:106-111`. |
| **OpenAlex** | Supported; reconstructs abstracts from inverted index and generates BibTeX. | `agent/tools/openalex_search.py:27-337`; enabled in `config/tools.json:112-117`. |
| **PubMed** | Supported; esearch + efetch XML parsing, MeSH/publication-type filters. | `agent/tools/pubmed_search.py:14-232`; enabled in `config/tools.json:118-123`. |
| **Google Scholar** | Implemented via `scholarly`, but **disabled by default**; warns about IP blocks. | `agent/tools/google_scholar.py:1-97`; `config/tools.json:124-129`. |
| **Semantic Scholar** | Implemented, but **disabled by default**; config exposes an API key. | `agent/tools/semantic_scholar.py:6-162`; `config/tools.json:130-135`; `config/schema.py:116-118`. |
| **Jina Reader** | HTML-to-Markdown service used by `web_fetch`, optional `JINA_READER_KEY`. | `agent/tools/web_reader.py:92-108`; `core/infra/config.py:69-70`, `108`. |

The `SearchProvider` abstraction has exactly one implementation; the docstring says future providers "such as Google, Bing or Serper" could be added, but there is no registry, no config selector beyond `Config.SEARCH_PROVIDER` defaulting to `"duckduckgo"`, and unknown names silently fall back to DuckDuckGo (`core/tools/web_search.py:28-46`, `95-106`; `core/infra/config.py:252`).

### 5.2 Rate limiting

- **arXiv:** module-level `threading.Semaphore(2)` plus a shared `arxiv.Client(delay_seconds=3.0, num_retries=3)`; HTTP calls are serialized under `_CLIENT_LOCK` so the 3-second delay is actually enforced (`agent/tools/arxiv_search.py:8-13`, `169-172`). On HTTP 429 it sleeps 20 seconds and retries twice (`agent/tools/arxiv_search.py:150-152`, `201-205`).
- **OpenAlex:** `threading.Semaphore(2)`, no delay and no retry (`agent/tools/openalex_search.py:6`, `132-139`, `188-193`).
- **PubMed:** `threading.Semaphore(2)`, no delay and no retry; NCBI API key is passed as a query parameter but no rate policy is enforced (`agent/tools/pubmed_search.py:7`, `118-125`, `163-164`).
- **Semantic Scholar:** no semaphore; a local retry loop of 3 attempts with linear sleep for 429/5xx (`agent/tools/semantic_scholar.py:58-66`, `145-160`).
- **DuckDuckGo:** no rate limit, no retry, no backoff (`core/tools/web_search.py:68-78`).
- **Global:** none. There is no token bucket, no provider-wide governor, no cross-process limit, and no per-job search budget.

These are per-tool class/module constants, not configurable policy.

### 5.3 Retries

- LLM calls: 3 attempts with exponential backoff `2**attempt` for timeout/429/5xx/connection errors (`providers/openai_provider.py:172-175`, `382-415`).
- arXiv: outer 429 retry as above.
- Semantic Scholar: 3 attempts, linear sleep.
- OpenAlex/PubMed/DuckDuckGo/Jina: no retry.
- Automation jobs: no retry at the job level. A failed run is recorded and the next cron fire is a fresh run (`core/automation/runtime.py:216-237`; `core/automation/models.py:119-132`).
- DAG tasks: `SDDExecutor` retries the worker/reviewer loop up to 3 times (`agent/scheduler/executor.py:27`, `67-97`). There is no retry of the whole job.

### 5.4 Caching

Only fetched documents are cached: PDFs under `~/.open_research_claw/cache/papers/` and other downloads under `~/.open_research_claw/cache/common/` (`agent/tools/web_reader.py:14-16`, `125-129`, `179-182`). Search results are **not cached**; repeated queries re-hit the provider. There is no HTTP cache, ETag handling, or TTL for search results.

### 5.5 Parallel querying

Parallelism exists at three levels:

1. **Within a turn:** all tool calls emitted by the LLM in one assistant message are run with `asyncio.gather` (`core/llm/engine.py:435-438`; `agent/loop.py:1147-1150`).
2. **Across DAG tasks:** `BatchRunner` gathers all ready tasks (`agent/scheduler/batch_runner.py:88-100`).
3. **Across processes/projects:** multiple automation jobs can fire concurrently because APScheduler runs each in its own asyncio task; per-job `max_instances=1` only limits the same job.

There is no query-level fan-out controller, no deduplication of in-flight identical queries, and no global concurrency budget. Per-tool semaphores are the only protection against provider overload.

---

## 6. Persistence and artifacts

### 6.1 Layout

`Project` defines `root = workspace_root / project_id`, `core = root / project_id`, and `memory_dir = root / ".project_memory"` (`core/project.py:683-689`). A session lives at `project.root / session_id`, with metadata under `.bot` (`core/session.py:85-91`).

Artifacts observed:

| Artifact | Path | Evidence |
|---|---|---|
| Paper/project files | `workspace/<project>/<project>/` (the "core") | `core/project.py:683-689` |
| Session metadata | `workspace/<project>/<session>/.bot/` | `core/session.py:89-91` |
| Task state | `.../.bot/task_state.json` | `agent/task_agent.py:90-95`; `agent/tools/task_tools.py:988-991` |
| Plan | `plans/latest_plan.json` and `plans/latest_plan.md` under the task project root | `agent/scheduler/planner.py:97-136` |
| Worker outputs | `core/_task_workers/<task_id>_r<round>/` | `agent/scheduler/executor.py:386-416` |
| Final merged outputs | project core (`report.md`, `paper.tex`, etc.) | `config/prompts/task_phase_finalize.txt:11-24` |
| Project memory index | `.project_memory/index.jsonl` | `core/memory/store.py:37-41` |
| Memory entries | `.project_memory/entries/MEM-XXXX.md` | `core/memory/store.py:185-186` |
| Compact memory index | `.project_memory/index_compact.md` | `core/memory/store.py:41` |
| Automation job definitions | `.project_memory/jobs/<id>.json` | `core/automation/store_fs.py:28-31` |
| Automation job state | `.project_memory/job_states/<id>.json` | `core/automation/store_fs.py:28-31` |
| Run history | memory entries with `kind="job_run"`, `scope="job:<id>"` | `core/automation/runtime.py:329-375` |
| Session history | `memory/history/chat_YYYY_MM.jsonl` | `agent/memory/logger.py:22-34`, `111-133` |
| Trajectories | `memory/trajectories/turn_*.json` | `agent/memory/logger.py:37-79` |
| Event trace | `memory/events/events_<session>.jsonl` | `agent/memory/trace.py:53-94` |
| Token usage index | `memory/trajectories/token_usage.jsonl` | `agent/memory/logger.py:58-75` |
| Fetched files | `~/.open_research_claw/cache/{papers,common}/` | `agent/tools/web_reader.py:14-16` |

There is **no SQL database in active use**. `core/infra/database.py` is an explicit skeleton: the docstring says chat tables were removed and only `DatabaseManager` remains "for future expansion" (`core/infra/database.py:1-29`). `Config.DB_PATH` is defined (`core/infra/config.py:271`) but no schema or migrations exist.

### 6.2 Schemas and semantics

The memory ledger is the most reusable schema. Each index record has:

`id` (`MEM-0001`), `kind`, `scope`, `intent`, `title`, `summary`, `tags`, `source`, `created_at`, `updated_at`, `ttl`, `parent_id`, `path` (`core/memory/store.py:188-204`).

Entries are Markdown files, the index is JSONL, and writes use a lock file + `fcntl.flock` (when available) + atomic replace (`core/memory/store.py:115-138`, `71-74`). The store supports hierarchical scopes (`job:<id>`, `project`, `user`), keyword search over metadata, pagination with a cursor, TTL parsing (`30d`, `12h`, `4w`), and lazy GC every 24 hours with protection for job-state references (`core/memory/store.py:30-32`, `96-109`, `405-565`, `646-727`).

This is a good **append-only run/evidence ledger shape**, but it stores prose notes, not typed claims with source spans.

### 6.3 Run resumability

- `TaskSession` can be saved to and loaded from `task_state.json` (`agent/task_agent.py:90-106`).
- On load, `RUNNING`/`REVIEWING` tasks are converted to `INTERRUPTED` (`agent/task_agent.py:165-172`).
- Recoverable tasks are `FAILED` or `INTERRUPTED` (`agent/task_agent.py:174-183`); `/resume` resets the selected task and its unfinished descendants to `PENDING`, with a `--force` option if completed downstream tasks would be reset (`agent/task_agent.py:236-281`).
- Interactive task execution persists state after each batch (`agent/tools/task_tools.py:716-718`, `836`).

Automation runs have **no resume**. A crashed run remains `running` in memory/state until overwritten; the next cron fire starts fresh. There is no durable queue, no lease, no idempotency key across restarts. The `JobRun` record is written only after execution returns (`core/automation/runtime.py:216-222`).

In-flight worker outputs live under `_task_workers/` and are deleted after commit (`agent/tools/task_tools.py:980-983`), so they are not durable history. The task state JSON is the only resume artifact.

---

## 7. Interfaces

### 7.1 HTTP API

FastAPI app titled "Research Claw Gateway" (`agent/services/gateway_server.py:28`). It exposes roughly 60 routes, including:

- Health: `GET /api/health` (`agent/services/gateway_server.py:273-276`).
- Config: `GET/POST /api/config`, backups/restore (`agent/services/gateway_server.py:586-965`).
- Projects: `GET /api/projects`, `POST /api/projects/refresh` (`agent/services/gateway_server.py:1670-1709`).
- Automation jobs: `GET/POST/PUT/DELETE /api/projects/{pid}/jobs`, `POST .../jobs/{job_id}/run` (`agent/services/gateway_server.py:1724-1898`).
- Runs: `GET /api/projects/{pid}/runs`, `GET/DELETE .../runs/{run_id}` (`agent/services/gateway_server.py:1737-1783`).
- Scheduler diagnostics: `GET /api/debug/scheduler` (`agent/services/gateway_server.py:1712-1721`).
- Push subscriptions and SMTP profiles (`agent/services/gateway_server.py:1190-1505`).
- Token usage analytics (`agent/services/gateway_server.py:2091-2117`).
- WebSocket chat: `/ws/chat` (`agent/services/chat_api.py:109-180`).

FastAPI would generate OpenAPI automatically, but there is no explicit API versioning, no request IDs, no pagination standard beyond ad-hoc `limit`/`cursor`, and no documented API contract.

### 7.2 CLI

Typer app with commands `login`, `reset`, `onboard` (hidden), `agent`, `subagent`, `gateway`, `stop`, `doctor`, `legacy_gateway` (`cli/main.py:221-258`, `325-353`, `678-774`, `864-936`). The primary entry points are `agent` and `gateway` (`README.md:265-277`). There is no `research` or `job` subcommand; jobs are managed through chat commands or the HTTP API.

### 7.3 MCP

**Absent.** A repository-wide search for `mcp` / `Model Context Protocol` across `*.py`, `*.md`, `*.json`, and `*.toml` returned zero matches. There is no MCP server, client, tool manifest, or transport.

### 7.4 Configuration

- Global `settings.json` in camelCase, generated from `settings.default.json` (`config/loader.py:14-49`, `91-120`).
- Tool registry: `config/tools.json`, class paths + args + enabled flags (`config/tools.json:1-250`).
- Agent profiles: `config/agent_profiles/*.json`, each with `role_type` and a tool allowlist (`agent/tools/loader.py:43-59`; `config/agent_profiles/automation_agent.json`).
- Prompts: `config/prompts/*.txt` with YAML frontmatter, rendered by `core/prompts.py`.
- Slash commands, modes, roles, i18n: `config/commands.json`, `config/modes.json`, `config/roles.json`, `config/i18n.json`.
- Per-project: `workspace/<project>/project.yaml` (Overleaf ID, LaTeX engine, Git, automation/radar config) (`core/project.py:80-92`, `734`).
- Secrets: API keys live in `settings.json` and per-provider instances (`config/schema.py:172-179`); some environment fallbacks exist (`core/infra/config.py:192-215`).

### 7.5 Auth

**No HTTP authentication or authorization.** No auth middleware, API key check, session check, or RBAC is registered. The only access checks are localhost guards on `POST /api/restart`, `/api/reset`, and `/api/update` (`agent/services/gateway_server.py:282-283`, `293-294`, `364-365`). Every other route, including config read/write and project/job management, is open to anyone who can reach the port. The default gateway host is `0.0.0.0` (`settings.default.json:8-11`; `config/schema.py:99-102`), despite the docs claiming it listens on `127.0.0.1` by default (`README/guide/webui_guide_en.md:644`).

IM channels have per-account `allow_from` lists (`config/schema.py:9-51`), but those are channel-level sender filters, not API auth.

A particularly bad example: `GET /api/chat/file?path=...` serves any absolute path that exists on the host (`agent/services/chat_api.py:76-85`), and `POST /api/chat/upload` writes `file.filename` without sanitization (`agent/services/chat_api.py:88-106`). Do not copy this pattern.

---

## 8. Safety

### 8.1 Prompt injection from fetched content

**No defense.** `web_fetch` returns page text directly as a tool result (`agent/tools/web_reader.py:96-108`), and the agent loop appends it to the conversation as a `tool` message (`core/llm/engine.py:473-487`). There is no `<untrusted>` wrapper, no instruction-stripping, no source-trust label, no "ignore instructions in fetched content" system prompt, and no output-side check. A repository-wide search for `prompt injection`, `untrusted`, `sanitize` (in the security sense), or `ignore instructions` found only message-history sanitization (`providers/base.py:81-118`) and truncation decorators (`core/llm/decorators.py:94`), not content-injection handling.

The only related guidance is prose: prompts tell the agent to verify that a PDF fetch returned real paper content (`config/prompts/project_active.txt:63-67`) and to ground BibTeX in search results (`config/prompts/project_active.txt:28`). That is not an enforcement boundary.

### 8.2 SSRF

**No SSRF protection.** `web_fetch` calls `requests.get(url)` on any URL the model supplies, with no scheme allowlist, no host resolution check, no private/link-local/metadata IP block, no redirect policy, and no DNS-rebinding defense (`agent/tools/web_reader.py:71-77`, `94-95`). The deprecated reader only checks `url.startswith("http")` (`core/tools/web_reader.py:78-81`). A model (or prompt-injected page) can fetch `http://127.0.0.1:*`, `http://169.254.169.254/`, or internal hostnames.

### 8.3 Secrets

- API keys and channel credentials are stored in plaintext in `settings.json` (`config/schema.py:172-179`; `config/loader.py:91-120`). There is no keyring, encryption, or file-permission hardening.
- `settings.json`, `.env`, and `.olauth` are gitignored (`.gitignore:17-22`), which prevents accidental commits but does not protect the file at rest.
- The UI masks SMTP passwords when returning config (`agent/services/gateway_server.py:596-600`), and provider logs mask API keys (`core/llm/providers.py:145-147`). That is display hygiene only.
- `config/tools.json` loads arbitrary class paths via `importlib`, so anyone who can edit it can execute code in the gateway process (`agent/tools/loader.py:91-120`). Treat config as trusted local input.

### 8.4 Sandboxing and path safety

- Worker sub-agents get a copy-on-init overlay and write there; `Session.resolve()` and `write_target()` reject `..` and absolute paths and block writes into protected dependency prefixes (`core/session.py:141-181`). This is a reasonable **cooperative** isolation model for file tools.
- But `bash` uses `subprocess.run(command, shell=True, cwd=working_dir)` with only a naive `".." in command` string check (`agent/tools/bash.py:74-79`, `112-119`). The cwd anchor does not prevent `cat /etc/passwd`, `curl` to internal hosts, `python -c ...`, or `git` operations (git is only blocked in task mode, `agent/tools/bash.py:54-58`). It is not a security boundary.
- `LocalEnv` adds heuristic checks for `rm/mv/cp/chmod/dd/>/sudo`, blocks absolute paths outside the sandbox when a "dangerous token" is present, and prompts the user interactively (`core/infra/envs/local.py:132-180`). This is bypassable and unsuitable for unattended daemon work.
- Docker and E2B environment classes exist (`core/infra/envs/docker_env.py`, `core/infra/envs/e2b_env.py`), and a Python audit hook can prompt on out-of-sandbox writes (`core/utils/audit_guard.py:1-92`). But the default tool wiring uses `agent/tools/bash.BashTool` with the session, not Docker/E2B (`config/tools.json:183-190`), so the sandbox classes are not on the normal path.
- The security guide claims "Bash command working directories are locked to core and cannot access external files" (`README/guide/04_Isolation_and_Security_en.md:11-14`). That claim is stronger than the implementation.

### 8.5 Other

- No outbound-domain allowlist, no URL reputation check, no content-size cap before `requests.get` reads the body (only after, at 100k chars for the returned text).
- No secret redaction in fetched content or logs beyond provider-key masking.
- No tenant isolation: projects are filesystem directories in one process; any code execution or path bug crosses projects.
- The `/api/update` route runs `git pull`/downloads a zip and `pip install` from the network, guarded only by a localhost check (`agent/services/gateway_server.py:355-504`). That is a remote-code-execution path if the port is exposed or the check is bypassed.

---

## 9. Maturity

### 9.1 Exact metadata

Commands run in the clone:

```
git log --oneline | wc -l                         -> 140
git log --reverse --format='%H %ad %an %s' --date=iso | head -1
  -> e77cfc2f89e0a1867f10061dd85ecdc3de338ffc 2026-03-14 13:17:39 +0800 cz
     Initial release: Open Overleaf Claw — AI-powered academic writing agent system
git log -1 --format='%H %ad %an %s' --date=iso
  -> ce3a4acd65d81f5f7681a90df0bf4abab288cc7d 2026-06-08 18:02:44 +0800 chengzhu.cz
     docs: 优化 README 头部区域布局
git tag -l                                        -> (empty)
git branch -a                                     -> main; remotes/origin/{main,2-webui-chat-support,feat/cstcloud-overleaf,zx_dev}
git shortlog -sn HEAD                             -> Hannah Parker 59, cz 40, chengzhu.cz 25, Poketjomon 14, zzcz 2
git log --format='%ad' --date=format:'%Y-%m' | sort | uniq -c
  -> 2026-03: 137, 2026-04: 1, 2026-06: 2
```

| Signal | Finding |
|---|---|
| Commits | 140 total, all on `main`/reachable branches. |
| First commit | 2026-03-14 (`e77cfc2`). |
| Last commit | 2026-06-08 (`ce3a4ac`), about five months before this survey. |
| Active window | Essentially March 2026: 137 of 140 commits. Then 1 in April, 2 in June. |
| Contributors | 5 names; top two account for 99/140 commits. |
| Tags / releases | None (`git tag -l` empty). `pyproject.toml` says version `0.1.0` (`pyproject.toml:3`). No GitHub release artifacts in the clone. |
| Branches | `main`; stale feature branches last touched 2026-03-28/29 (`2-webui-chat-support`, `feat/cstcloud-overleaf`, `zx_dev`). |
| CI | **None.** No `.github/` directory and no workflow YAML anywhere (`find` for `*.yml`/`*.yaml` outside tests/config returned nothing). |
| Tests | 20 Python test files, 52 `def test_` functions. Most cover channels (DingTalk/Telegram), reply quoting, and provider token handling (`tests/channels/*`, `tests/test_reply_quote*.py`, `tests/providers/test_openai_provider_tokens.py`). |
| E2E suite | 34 YAML scenarios under `tests/cases/` for writing (`w*`), research (`r*`), memory (`m*`), and radar (`mr*`). They run full agent sessions against a live LLM and, for many cases, a live Overleaf account (`tests/README.md:15-40`, `102-156`). Evaluation is an LLM judge invoked through the Claude Code CLI (`tests/scripts/judge_eval.py:18-33`). |
| Harness checks | A small memory/radar replay harness with shell assertions (`tests/harness/README.md:1-44`). |
| Test execution in this survey | Not run: `python3` is not installed in the survey environment (`python3 --version` -> `command not found`). No dependency install was attempted. |
| Docs | Strong user docs: 20k-char English README, 20k-char Chinese README, 9 bilingual guides, IM setup guides, push-subscription guides. Weak architecture docs: `CLAUDE.md` refers to `core/fs/` and `agent/wide_loop.py`, neither of which exists (`CLAUDE.md:36`, `42`; `ls core/fs` and `ls agent/wide_loop.py` both fail). The overview admits "scheduling feature not yet verified" (`README/guide/01_Overview_en.md:13`). |
| License | MIT, copyright 2025 ContextBot Contributors (`LICENSE:1-3`). Permits reuse with attribution. |
| Repo hygiene | `tests/` is listed in `.gitignore` (`.gitignore:48`) yet 58 test files are tracked — a sign of rushed configuration. No lockfile, no formatter/linter config, no pre-commit, no type checking. |

### 9.2 Maintenance signals and abandoned areas

- The commit curve is a three-week sprint followed by near-silence. This looks like a demo, competition, or article companion rather than a maintained product.
- `browser_use` is implemented but disabled (`config/tools.json:229-235`).
- Google Scholar and Semantic Scholar tools are implemented but disabled (`config/tools.json:124-135`).
- Brave web-search configuration is plumbed but unused (see §5.1).
- `core/infra/database.py` is an explicit skeleton with no schema (`core/infra/database.py:1-29`).
- Scheduler concurrency is a TODO (`agent/scheduler/engine.py:49`).
- Security docs overstate the bash sandbox (`README/guide/04_Isolation_and_Security_en.md:11-14` vs `agent/tools/bash.py:74-79`, `112-119`).
- `CLAUDE.md` is stale relative to the tree.
- No releases, no issue templates, no CI, no changelog.

---

## 10. Fit for `research-daemon`

### 10.1 What is worth borrowing (design, with paths)

**A. Task/job state machine and DAG model.**
`agent/scheduler/schema.py:21-91` is a compact, readable model: typed statuses, dependencies, acceptance spec, assigned agent, output directory, artifacts, iteration budget, reviewer feedback, retry count. For `research-daemon`, extend it with `query_plan`, `evidence_ids`, `provider`, `budget`, `deadline`, and `lease`. The status set (`pending/running/reviewing/completed/failed/blocked/interrupted`) maps cleanly to a Rust enum. This is the single most directly translatable artifact.

**B. Ready-set batch scheduling.**
`agent/scheduler/batch_runner.py:52-113` is a clean "execute all tasks whose dependencies are complete, then return control" loop. It is small enough to port to Rust in an afternoon. Add what it lacks: a global semaphore, per-provider rate limits, and a batch-size cap.

**C. Resume semantics.**
`agent/task_agent.py:165-281` has the right instincts: convert stale in-flight states to `INTERRUPTED` on load; expose recoverable tasks; reset a failed task and its unfinished descendants; require `--force` if completed descendants would be reset. This is a good template for durable run recovery.

**D. Filesystem memory/evidence ledger.**
`core/memory/store.py:30-74`, `162-209`, `405-565`, `646-727`:
- one JSONL index record per entry, one Markdown body per entry;
- `kind`, `scope`, `intent`, `title`, `summary`, `tags`, `source`, `ttl`, `parent_id`;
- atomic temp-file replace and `flock`;
- cursor pagination and scoped listing;
- TTL parsing and lazy GC with protection for referenced job state.

For `research-daemon`, replace prose `content` with typed evidence records: `claim_id`, `source_url`, `source_title`, `retrieved_at`, `quote`, `content_hash`, `confidence`, `query_id`, `run_id`. Keep the append-only JSONL + body-file split if you want human inspection; use SQLite only if you need transactional queries.

**E. Automation job/state separation and run records.**
`core/automation/store_fs.py:24-61` separates job definitions (`jobs/*.json`) from state (`job_states/*.json`) and writes atomically. `core/automation/models.py:61-148` shows a useful `JobRun` record with trigger, status, timestamps, output excerpt, error, and token usage. `core/automation/runtime.py:329-375` shows how to write a run note into the ledger and deduplicate an agent-written run entry. This is a good "run ledger" pattern, even if the storage should become a database in Rust.

**F. Cron scheduler settings.**
`core/automation/scheduler_aps.py:88-106` is a good default policy: coalesce missed runs, one instance per job, one-hour misfire grace, and a heartbeat. Copy the policy, not the Python.

**G. Provider-specific pacing.**
`agent/tools/arxiv_search.py:8-13`, `150-206` is the only place in the repo with real rate-limit thinking: a shared client, a persistent delay, a semaphore, and a 429 backoff. Generalize this into a per-provider `RateLimiter` trait in Rust (token bucket + concurrency semaphore + retry policy + circuit breaker). The OpenAlex/PubMed semaphores (`agent/tools/openalex_search.py:6`, `agent/tools/pubmed_search.py:7`) are a weaker version of the same idea.

**H. Research prompts and skills (copyable assets).**
These are the most directly reusable files because they are data, not code:
- `config/prompts/skill_radar.txt:50-77` — job-state cutoff, multi-source minimum, three search rounds, query diversification, PDF verification, "do not relax the cutoff".
- `config/.skills/paper-research-form/SKILL.md:20-60` — clarify scope, search and verify metadata, build rows, merge/append, deliver.
- `config/.skills/paper-research-form/references/search-strategy.md:21-93` — source priority, query patterns, selection rules, metadata verification, relevance rubric.
- `config/.skills/paper-research-form/references/csv-schema.md` — a concrete 8-column evidence/summary schema with ordering and dedupe rules.
- `config/.skills/paper-research-form/references/merge-and-delivery.md:33-68` — normalized-title dedupe, published-over-preprint preference, delivery gate.
- `config/.skills/ml-paper-writing/references/citation-workflow.md:39-85` — the "verify in 2+ sources, retrieve BibTeX, validate claim" workflow.
- `config/prompts/project_active.txt:28`, `63-67` — ground BibTeX in tool results; verify PDF fetch success.

These can be stored as prompt templates or converted into deterministic pipeline stages. Their value is the **method**, not the Python.

**I. Scenario-based evaluation.**
`tests/cases/*.yaml` plus `tests/scripts/run_batch.py:52-99` and `tests/scripts/judge_eval.py` show a useful acceptance-test format: a case declares project, mode, query, timeout, and expected outcomes; the runner archives the workspace, runs the agent, and optionally asks an LLM judge to grade the trace. For Rust, keep the YAML case shape and replace the judge with deterministic assertions plus optional LLM grading.

### 10.2 What to avoid

1. **Do not port the search stack.** DuckDuckGo-only, no Tavily/Kagi, a dead Brave key, no retries/cache on web search, and no global rate limiter (`core/tools/web_search.py:48-106`; §5). Build a provider trait with Tavily first and a real limiter.
2. **Do not use `web_fetch` as a security model.** Raw `requests.get`, no SSRF guard, no prompt-injection boundary (`agent/tools/web_reader.py:71-108`). In Rust, enforce scheme/host allowlists, resolve and reject private/link-local/metadata IPs, cap redirects and body size, and wrap fetched content in a clearly delimited untrusted block.
3. **Do not copy the bash tool.** `shell=True`, a `".."` string check, and cwd anchoring are not isolation (`agent/tools/bash.py:74-79`, `112-119`). If jobs need shell, run them in a container/namespace with an explicit mount and network policy; otherwise omit shell from the research daemon.
4. **Do not copy the HTTP surface.** No auth, open config/job routes, arbitrary absolute-path file download, unsanitized upload (`agent/services/chat_api.py:76-106`; §7.5). Use authenticated API keys, per-tenant scoping, and a workspace-root path resolver.
5. **Do not copy plaintext secret storage.** `settings.json` with API keys (`config/schema.py:172-179`). Use OS keyring or an encrypted secret store, and never return secrets from read APIs.
6. **Do not adopt the unbounded fan-out.** `asyncio.gather` over all ready tasks (`agent/scheduler/batch_runner.py:100`) and no scheduler concurrency limit (`agent/scheduler/engine.py:49`). Put a global job semaphore and per-provider limiter in front of every fan-out.
7. **Do not rely on LLM-judged review as the only gate.** The reviewer emits `CONCLUSION: PASS/FAIL` from prose (`agent/scheduler/executor.py:493-590`). Pair it with deterministic checks (schema validation, URL liveness, citation existence, duplicate detection).
8. **Do not use `latest_plan.json` as the plan store.** It overwrites the previous plan (`agent/scheduler/planner.py:109-136`). Keep immutable plan versions keyed by run id.
9. **Do not copy the monolithic gateway.** 2,165 lines of routes, config, jobs, backups, SMTP, and static UI in one module (`agent/services/gateway_server.py`). Split API, job service, and storage in Rust.
10. **Do not trust the docs' security claims.** The guide says bash is locked to the project (`README/guide/04_Isolation_and_Security_en.md:11-14`), but the implementation is not.
11. **Do not inherit the dependency weight.** 42 unpinned Python dependencies including browser automation, pandas, Docker, and IM SDKs (`requirements.txt:1-42`). A Rust daemon should have a small, auditable dependency set.

### 10.3 Direct code reuse vs design reuse

**Directly reusable (copy as data, with attribution):**
- Prompt/skill Markdown under `config/.skills/` and `config/prompts/` (MIT).
- The JSON shape of `TaskGraph`/`ResearchTask` as a starting schema (`agent/scheduler/schema.py:21-91`).
- The YAML shape of `tests/cases/*.yaml`.
- The memory record field set (`core/memory/store.py:188-204`) and `JobRun` field set (`core/automation/models.py:119-148`).
- The BibTeX generation logic is small and portable in concept (`agent/tools/openalex_search.py:288-337`), but trivially reimplemented.

**Not directly reusable:** all Python runtime code. There is no Rust crate, no shared library, no language-neutral IDL. Every component must be reimplemented.

### 10.4 Rough porting effort

Assuming a Rust workspace with API + CLI + MCP, a Tavily provider, SQLite/Postgres, and a durable job queue:

| Piece | Effort | Notes |
|---|---:|---|
| Domain model (`Job`, `Run`, `QueryPlan`, `Evidence`, `Artifact`) | 2–4 days | Translate `schema.py` + `models.py`, add query/evidence types. |
| DAG executor + batch scheduler + global concurrency | 4–6 days | Port `BatchRunner`/`SchedulerEngine`; add semaphores, retries, leases. |
| Durable persistence + resume | 1–2 weeks | Replace JSON files with SQLite/Postgres; implement run/lease/recovery semantics inspired by `task_agent.py:165-281`. |
| Provider trait + Tavily + rate limiter + cache | 1–2 weeks | New code; borrow the arXiv pacing idea (`arxiv_search.py:8-13`, `150-206`). |
| Evidence ledger + citation binding | 1–2 weeks | New code; use the memory record shape and TTL/GC ideas. |
| API + CLI + MCP surfaces | 1–2 weeks | New code; keep the route set small. |
| Prompt/skill port | 2–4 days | Copy Markdown, parameterize, add deterministic validators. |
| Integration/e2e harness | 3–5 days | Reuse YAML case shape and archive-on-failure pattern. |

**Total:** roughly **6–10 weeks** for one engineer to reach a credible MVP, with perhaps 1–2 weeks saved by reusing the prompt assets and schema ideas. The Python code itself saves almost nothing.

---

## 11. Verdict

**Mine for parts — reference only for code.**

`research-claw` is the closest prior art in this set for the *shape* of a research job system: a DAG of typed tasks, dependency-aware batch execution, resumable task state, cron-driven recurring research, a filesystem ledger with scopes/TTL, and prompt-level query discipline. Those ideas are worth translating into Rust, and the skill/prompt Markdown is worth copying under MIT.

It is not a foundation to adopt. It is a 40k-LOC Python monolith with a three-week commit history, no CI, no releases, no MCP, no real web-search provider beyond DuckDuckGo, no global rate limiting or caching, no HTTP auth, no SSRF protection, and no prompt-injection boundary. Its scheduler is a single-process DAG evaluator, not a durable queue. Its "citations" are a flat markdown list appended to answers, not an evidence ledger. Treat it as a **design quarry**, not a dependency.
