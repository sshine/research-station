# Survey: `laoujin-scout`

- **Repository:** `/home/sshine/Projects/nursery/research-station/external-research-agents/laoujin-scout`
- **Remote:** `git@github.com:Laoujin/Scout.git` (`git remote -v`)
- **Commit surveyed:** `4bf889d` (`fix AskUserQuestions`, 2026-07-15)
- **Survey date:** 2026-11-02
- **License:** MIT (`LICENSE:1-21`)

## 1. Purpose and problem

Scout is a **personal, self-hosted research engine** built around GitHub Issues and Claude Code. The README's one-line flow is: "Open a GitHub Issue → Claude researches on your hardware → cited result published to your own Jekyll site via GitHub Pages" (`README.md:9-10`). It is a fork-and-operate product, not a library: the repo is "self-hosted GitHub Actions runner + research workflow + skill pack" (`README.md:29`), with two sibling repos: Atlas (the Jekyll publishing target) and Compass (the theme) (`README.md:27-31`).

The problem it solves is not "run a search" but **turn a vague question into a durable, cited, publishable research artifact with low operator effort**, including from a phone through a GitHub Issue (`README.md:80-87`). It deliberately trades determinism and security for a very small amount of glue code around a capable agent.

**Target user:** one technical self-hoster with an always-on Linux box, Docker, a GitHub account, and Claude Code auth (subscription or API key) (`README.md:33-34`). There is no multi-user model, no tenancy, no hosted service. The `profile.yml` feature personalizes query sharpening with location/language/currency/interests (`profile.example.yml:1-14`) but is still single-operator.

It is **not** a daemon, an API server, an MCP server, or a reusable research library. Those surfaces are absent; see §3 and §7.

## 2. Language, stack, build system, dependencies, install/run

**Language/stack:** Bash dominates (32 shell scripts, ~3,855 lines), plus one Python 3 script for Atlas triage (`skills/scout-triage/scan.py`, 442 lines), Markdown "skill" prompts, GitHub Actions YAML, and a Dockerfile. There is no compiled code, no package manifest, no Cargo/npm project for the core, and no versioned binary. The repository is mostly a **prompt pack plus shell orchestration**.

**Build system:** there is effectively no build. "Install" is `install.sh`, which builds a disposable Alpine installer container and then a runtime image; runtime is `docker-compose up -d --build` (`README.md:38-58`, `INSTALL.md:1-170`). Local plugin usage has no build step: `/plugin marketplace add <you>/Scout` then `/plugin install scout@scout` (`README.md:90-108`). Tests are plain `bash tests/test_*.sh` and are not wired into a build target.

**Dependency weight:** heavy for a shell project, because it packages a full agent runtime:
- `node:22-bookworm-slim` base; apt: git, curl, OpenSSH, jq, python3, ImageMagick, webp, Chromium runtime libraries, fonts (`docker/Dockerfile:1-13`).
- GitHub CLI `gh` (`docker/Dockerfile:15-25`).
- Global npm: `@anthropic-ai/claude-code` and `playwright`; then Chromium (`docker/Dockerfile:31-38`).
- GitHub Actions runner binary, version 2.321.0 (`docker/Dockerfile:40-44`).
- External services: GitHub, Anthropic/Claude, arbitrary web pages, GitHub API (stars), Wikimedia Commons (covers), Atlas git remote.

**Install/run story:**
1. `curl -fsSL .../install.sh | bash -s -- --config=s5.cartography.v1` forks Scout, creates Atlas, registers a self-hosted runner (`README.md:38-58`).
2. `cd Scout/docker && docker-compose up -d --build`.
3. One-time `docker exec -it scout-runner runuser -u runner -- claude`.
4. Open a research Issue, or run `/scout` locally (`README.md:60-78`, `README.md:90-108`).

This is a **personal appliance install**, not a `cargo install`-style developer dependency. The operational surface (runner token rotation, Claude re-auth, deploy keys, Docker volumes) is documented in `docs/OPERATE.md:1-100` and `INSTALL.md`.

## 3. Architecture: components, process model, job model, scheduling, concurrency

### Components

1. **GitHub Issue Form / workflow_dispatch** — input surface (` .github/ISSUE_TEMPLATE/research.yml:1-40`, `.github/workflows/research.yml:18-34`).
2. **GitHub Actions workflows** — event routing and job timeouts (`research.yml`, `triage.yml`, `pages.yml`).
3. **Shell orchestrators** — `scripts/run.sh` (single run), `scripts/run-decompose.sh` (parent expedition), `scripts/research-from-issue.sh` (routing), `scripts/sharpen.sh`, `scripts/publish.sh`.
4. **Claude Code headless** — invoked as `claude --dangerously-skip-permissions --print --output-format json --append-system-prompt <skill>` (`scripts/run.sh:150-156`). The "research engine" is an LLM session, not deterministic code.
5. **Skill/agent prompts** — `skills/scout-research/SKILL.md`, `deep.md`, `sharpen.md`, `synthesis.md`; subagents in `.claude/agents/*.md`.
6. **Atlas git checkout / worktree** — the artifact store and publish target (`scripts/local-setup.sh:25-47`, `scripts/run.sh:90-118`).
7. **GitHub Pages/Jekyll** — renders frontmatter and markdown/HTML (`README.md:29-31`).

### Process model

**No daemon or long-running server.** Execution is either:
- **CI:** GitHub Actions dispatches a job to a self-hosted runner; one job runs one shell pipeline and exits (`.github/workflows/research.yml:55-312`).
- **Local:** a Claude Code slash command runs the research in the interactive session; shell helpers set up a git worktree and publish (`skills/scout/SKILL.md:1-170`).

There is no scheduler, no queue, no worker pool, no job database, no RPC, and no API process. The nearest thing to a queue is GitHub's Actions queue plus the per-issue concurrency group (`research.yml:39-41`).

### Job model

Workflow jobs:
- `sharpen-on-open` — on new owner/member Issue; 10-minute timeout (`research.yml:55-63`).
- `resharpen-on-comment` — on owner/member reply; 10-minute timeout (`research.yml:102-112`).
- `research` — on edited bot comment with `Start research` checked; 270-minute timeout (`research.yml:161-174`).
- `views` — author HTML views; 270-minute timeout (`research.yml:209-218`).
- `rerun` — re-run failed sub-topics; 270-minute timeout (`research.yml:245-256`).
- `research-dispatch` — manual dispatch; 270 minutes (`research.yml:289-291`).

Dispatch routing is by string matching on the issue/comment body: `scripts/lib-issue-parse.sh` extracts topic/depth/options and sub-topics, with fuzzy depth-token snapping (`lib-issue-parse.sh:100-190`); `scripts/research-from-issue.sh:20-70` chooses decomposed vs single-pass. This is fragile but explicit and testable; see the parser tests.

### Scheduling and concurrency

- **GitHub Actions** provides scheduling/queueing. A daily cron runs the Atlas triage (`triage.yml:5-8`), but it is an audit, not research.
- **Concurrency group** is `research-${{ issue.number || github.ref }}` with `cancel-in-progress: false` (`research.yml:39-41`), so two research jobs for the same issue serialize; different issues can run concurrently on separate runners.
- **Local parallel runs** use per-run git worktrees, a unique slug checked against `origin/main`, live branches, and live worktrees, with `git worktree add` as the atomic backstop (`local-setup.sh:29-44`).
- **CI decomposed runs are sequential**, not parallel: `run-decompose.sh:234-309` is a plain `for entry in "${CHILDREN[@]}"` loop that invokes `run.sh` once and waits (`run-decompose.sh:265`). This directly contradicts `README.md:86` ("each runs as its own expedition in parallel") and the `/scout` local flow, which does dispatch children in one message (`skills/scout/SKILL.md:142-154`). The shell `views-dispatch.sh` *does* fan out with `&`/`wait` (`views-dispatch.sh:126-175`).
- **Hard caps:** max 8 children (`run-decompose.sh:26`), max 6 concurrent researchers per deep parent (`skills/scout-research/deep.md:21,44`), one reviewer pass, one remediation round with at most 2 researchers (`deep.md:31-48`).

## 4. Research pipeline

### Phases

1. **Intake** — Issue body or dispatch input: topic, depth (`recon`/`survey`/`expedition`), optional skip-sharpen (`research.yml:18-34`; `lib-issue-parse.sh:78-100`).
2. **Sharpen** — an LLM rewrites the raw topic into a verb-led brief with Scope/Compare/Constraints/Output bullets, preserving steering hints verbatim (`skills/scout-research/sharpen.md:8,38-76,78-92`).
3. **Human gate** — the bot posts the brief plus `- [ ] **Start research**` and optional sub-topic checkboxes (`scripts/issue-comment.sh:60-95`). The user ticks to run or replies with feedback to re-sharpen (`README.md:82-86`).
4. **Plan/decompose** — for `expedition`, sharpen proposes 2-8 sub-topics, with up to 3 *unticked* "completeness sweep" suggestions; the user selects (`sharpen.md:82-107`). Deep runs also write `outline.md` with 3-8 sub-questions (`deep.md:9-17`).
5. **Research** — single pass or parallel researchers; each researcher searches, fetches, extracts claims into a per-agent JSONL ledger, and returns a compressed summary (`scout-researcher.md:20-55`).
6. **Reflect/requery** — standard does one gap-driven round; deep researchers self-reflect, then one remediation round after review (`SKILL.md:199`; `deep.md:31-36`).
7. **Draft/synthesize** — parent writes the artifact from summaries + merged ledger; for an expedition, parent writes 200-600 words of cross-cutting synthesis and a `children:` grid (`synthesis.md:23-41`; `deep.md:25-27`).
8. **Review** — adversarial reviewer returns line-numbered blocking issues, coverage gaps, prose nits; parent applies one pass (`scout-reviewer.md:19-56`; `deep.md:28-37`).
9. **Validate + stamp metadata** — ledger validation, frontmatter YAML validation, deterministic cost/duration/model injection (`run.sh:224-258`).
10. **Publish** — commit/push to Atlas, comment the Pages URL; optional HTML views and triage afterward (`run.sh:313-315`; `publish.sh:1-105`).

### Query generation and decomposition

- Sharpen explicitly reframes noun phrases into verb-led deliverables, adds the current year for temporal topics, surfaces 1-3 comparison axes, preserves steering hints verbatim, and matches scope to depth (`sharpen.md:38-76`).
- The **completeness sweep** is a notable design idea: it proposes facets the user did not ask for (security/deployment for a "build X" brief; total cost/lock-in for a "choose X" brief) as opt-in, never as invented constraints (`sharpen.md:86-92`).
- Deep planning uses a breadth heuristic: decision criteria, producer claims, skeptic counter-reading, operator experience, alternatives, history (`deep.md:9-16`). This is a **prompt-level plan**, stored as `outline.md` one line per sub-question (`deep.md:17`), not a typed DAG or query graph.
- Every search query must include the literal year from `DATE` to avoid stale training-cutoff queries (`SKILL.md:196`).

### Source selection

- Topic-kind rubric: software → Reddit/HN/GitHub/YouTube; hardware → Reddit/vendor specs/benchmarks; local → Michelin/TripAdvisor/Yelp/local blogs; products → Wirecutter/RTINGS; research → blogs/Twitter/arXiv/Scholar/talks; news → major outlets (`SKILL.md:56-69`).
- Source-type taxonomy: `official`, `peer-reviewed`, `vendor-blog`, `forum`, `news`, `wiki` (`SKILL.md:71-84`), enforced in the ledger validator (`validate_ledger.sh:46-52`).
- No programmatic source-ranking or credibility score; the model chooses.

### Fetching and extraction

- Primary: Claude Code `WebSearch` and `WebFetch` (`SKILL.md:196`; `scout-researcher.md:4,45`).
- Fallback for empty/JS-walled pages: `npx playwright chromium -o rendered.html <url>` then read the rendered file (`SKILL.md:196`; `scout-researcher.md:45`).
- Extraction is entirely model-driven: the researcher writes one ledger line per usable claim with a verbatim quote ≤300 chars (`scout-researcher.md:31-48`). There is no Readability/HTML parser, no boilerplate removal, no per-domain extractor, no content hash, and no server-side quote verification.
- Images/og:image use a separate `scripts/fetch-image.sh` (curl + ImageMagick + Wikimedia Commons search) for covers/views (`fetch-image.sh:1-98`).

### Synthesis and report structure

- The artifact must open with a cited TL;DR/Decision block; comparisons should be tables; prose is terse; no trailing References (`SKILL.md:101-146`).
- Citations are inline: markdown `[[n]](url)`, HTML `<sup><a href="url">[n]</a></sup>` (`SKILL.md:110-113`, `CLAUDE.md:9-20`). "If a claim has no URL, do not make the claim" (`SKILL.md:136`).
- Frontmatter is a fixed contract: `title`, `date`, `depth`, `format`, `topic`, `topic_raw`, optional `issue`, `tags`, `summary`, `citations`, `reading_time_min`, plus injected `cost_usd`, `duration_sec`, `model`, optional `cover` (`SKILL.md:148-182`; `run.sh:242-258`).
- Expedition synthesis must disclose failed/skipped children, avoid re-summarizing children, cite original URLs rather than child pages, and skip synthesis if fewer than 2 children succeeded (`synthesis.md:23-33`; `run-decompose.sh:374-395`).
- Length targets: recon ~400 words/5-8 citations, survey 2-4 pages/15-30 citations, expedition as long as needed/40+ citations (`SKILL.md:90-97`).

### Stopping criteria

- Per researcher: stop after 5 distinct URLs unless coverage is incomplete; hard cap 8 searches / 12 URLs; status `done`, `done-partial`, or `blocked` (`scout-researcher.md:47,51-55`).
- Deep parent: max 6 initial researchers, max 2 remediation researchers, exactly 1 reviewer pass, exactly 1 illustrator call (`deep.md:42-48`).
- Decompose: max 8 children, 4h soft timeout, 4h20m hard timeout (`run-decompose.sh:26-28,246-265`).
- CI: 270-minute job timeout (`research.yml:174`).
- No token/cost budget enforcement: cost is measured *after* the run from Claude's result JSON (`run.sh:245-258`), and the README/FUTURE acknowledge budget caps as future work (`FUTURE.md:80`). No adaptive "stop when marginal evidence is low" beyond the model's self-assessment.

## 5. Search providers, rate limiting, retries, caching, parallel querying

### Providers

**Scout does not support Tavily, Kagi, SerpAPI, Brave, Bing, DuckDuckGo, or any pluggable search API.** An audit of the tree found no provider names or keys:

```bash
grep -rniE 'tavily|kagi|serp|brave|bing|duckduckgo|search.*api' \
  --include='*.sh' --include='*.md' --include='*.yml' --include='*.json' .
# no search-provider hits (only unrelated words like "serp" absent)
```

Search is whatever Anthropic's `WebSearch` tool does server-side, plus model-directed `WebFetch` and a Playwright fallback. The only direct HTTP APIs are:
- `https://api.github.com/repos/{owner}/{repo}` for star counts (`SKILL.md:133`);
- `https://commons.wikimedia.org/w/api.php` for cover images (`fetch-image.sh:18,36-46`);
- arbitrary `og:image` URLs (`fetch-image.sh:48-68`).

There is no provider interface, no API-key config, no query budget, no result normalization, and no fallback provider.

### Rate limiting

**No rate limiting in Scout.** The only rate limit mentioned is GitHub's unauthenticated 60 req/h vs 5,000 req/h with `GH_TOKEN` (`SKILL.md:133`), and that is informational, not enforced. There is no token bucket, no per-provider concurrency limit, no backoff on 429/503, and no queue depth control. The deep researcher cap (6) is a *concurrency* cap, not a rate limit.

### Retries

- Search/fetch: **no retry logic**. A failed `WebFetch` falls back to Playwright once (`SKILL.md:196`); after that the model decides.
- Publish: `publish_path` retries a non-fast-forward push with rebase up to 3 times with 1s/2s/4s sleeps, then falls back to a branch + compare URL (`lib-publish.sh:92-131`).
- Claude errors: `run.sh` classifies usage/rate-limit/overload errors into a human message and exits, but does not retry (`run.sh:45-55`). There is no exponential backoff around the `claude` invocation.
- Decompose children: per-child timeout, then placeholder/annotation; no retry (`run-decompose.sh:265-301`).

### Caching

- No HTTP cache, no ETag/Last-Modified handling, no response store, no DB.
- The only cache is an in-agent convention: "one GET per distinct repo, cache by `owner/repo`" for GitHub stars (`SKILL.md:133`) — implemented by the model in context, not by code.
- Ledger deduplication by URL is a post-hoc merge, not a fetch cache (`merge_ledgers.sh:19-30`).

### Parallel querying

- Deep runs: up to 6 researcher subagents in parallel, one per sub-question (`deep.md:18-21,44`). This is parallel *sub-question* research, not parallel query fan-out within one sub-question; each researcher runs 2-4 initial searches and up to 8 total (`scout-researcher.md:44-47`).
- Local `/scout` expedition: all child agents dispatched in one message (`skills/scout/SKILL.md:142-154`).
- CI decompose: **sequential** (`run-decompose.sh:234-309`), despite README claims. This is a real architectural gap if ported.
- Views: shell fan-out with `&` and `wait` (`views-dispatch.sh:126-175`), with per-slug logs but no concurrency cap.

## 6. Persistence and artifacts

**There is no database.** State is files in a git worktree plus GitHub issue comments. This is both the durability model and the audit trail.

### Workspace layout

For a run, `run.sh` clones Atlas (or reuses a parent-created dir) and creates:

```
<atlas>/research/<DATE>-<SLUG>/
  index.md | index.html          # canonical artifact
  citations.jsonl                # merged evidence ledger (standard/deep)
  citations.a<N>.jsonl           # per-researcher ledgers (deep)
  citations.urls.txt             # URL list for EXCLUDE_URLS
  outline.md                     # deep query plan (one sub-question per line)
  manifest.json                  # expedition child manifest
  cover.svg                      # optional cover
  .scout-result.json             # Claude CLI result JSON (cost/duration/model)
  .scout-error                   # structured failure reason
  .view-candidacy.json           # HTML-view judgement
  views/<view_name>.html         # optional bespoke views
  <images/data>                  # supporting assets
```

Sources: `run.sh:90-118,142-164`; `deep.md:17,73-76`; `run-decompose.sh:196-231,548-553`; `skills/scout/SKILL.md:186-220`.

### Schemas

**Citation ledger** — JSON Lines, one object per line:

```json
{"n":1,"url":"https://example.com","claim":"one-sentence claim","source_type":"official|peer-reviewed|vendor-blog|forum|news|wiki","quote":"verbatim snippet ≤300 chars","github_stars":52000}
```

Defined at `SKILL.md:137-141` and `scout-researcher.md:31-39`. `n` is local to each per-agent ledger, then renumbered at merge; `github_stars` is required for GitHub URLs; duplicate URLs are tolerated by validation but deduped at merge (`validate_ledger.sh:54-56`; `merge_ledgers.sh:19-30`).

**Manifest** — `manifest.json` is a JSON array of `{slug,title,depth,status,start,end}` (`run-decompose.sh:196-231`). It is the durable record of expedition children and is re-emitted from memory before publish because agents were observed truncating it (`run-decompose.sh:548-553`).

**Frontmatter** — see §4; `status: failed`, `failure_reason`, `validation_error`, `review_status` are used for degraded/placeholder pages (`run-decompose.sh:138-175`; `deep.md:48`).

### Run resumability

- A decompose parent **skips children that already have a non-placeholder `index.md`/`index.html`** (`run-decompose.sh:242-244`; `_child_is_success` at `run-decompose.sh:124-136`).
- Each successful child is pushed immediately, so a mid-expedition crash does not lose finished work (`run-decompose.sh:38-69,286-295`).
- Failed children get a placeholder with `status: failed` and a reason; children with content but a failed run get `validation_error` and are still published (`run-decompose.sh:286-301`).
- Rerun is offered as a one-click comment for failed sub-topics (`run-decompose.sh:573-579`; `scripts/rerun-from-issue.sh`).
- There is **no run ID, no idempotency key, no transaction, and no explicit checkpoint file** beyond the artifacts. Resume is "does this directory contain real content?" A rerun after a partial write can duplicate or overwrite depending on path.

### Output location

Artifacts land under `research/<DATE>-<SLUG>/` in Atlas and are served at `https://<owner>.github.io/<Atlas>/research/<DATE>-<SLUG>/` (`publish.sh:55-60`). The repo itself does not store research outputs; the Jekyll site does. Cost/duration/model are stamped into frontmatter; `.scout-result.json` ships as the raw provenance record.

Triage is a separate read-only audit: `skills/scout-triage/scan.py` walks `atlas/research/`, reads frontmatter + `manifest.json` + `.scout-result.json`, and classifies `DEAD`, `GENUINE_FAILURE`, `FALSE_FLAG`, `MANIFEST_MISMATCH`, `MISSING_COST`, `LEDGER_MISMATCH`, image issues, etc. (`scan.py:1-80`; `skills/scout-triage/SKILL.md:1-80`). This is a useful model for a daemon's artifact health check.

## 7. Interfaces: HTTP API, CLI, MCP, config, auth

### HTTP API

**Absent.** No HTTP server, routes, JSON-RPC, OpenAPI, or client SDK. Audit:

```bash
grep -rniE 'http server|localhost:|listen|axum|actix|flask|fastapi|express|json-rpc|mcp server' \
  --include='*.sh' --include='*.py' --include='*.md' --include='*.yml' .
# only "MCP" as a research topic in prompt examples, no server
```

### CLI

There is a **script CLI**, not a packaged CLI:
- `scripts/run.sh` — single run, env-driven (`TOPIC`, `DEPTH`, `FORMAT`, `RESEARCH_DIR`, `ISSUE_NUMBER`) (`run.sh:1-18,120-132`).
- `scripts/sharpen.sh` — reads `RAW_TOPIC`, `DEPTH`, optional previous/feedback/profile/series; prints sharpened brief (`sharpen.sh:1-70`).
- `scripts/run-decompose.sh` — reads `PARENT_DIR`, `PARENT_TOPIC`, `DATE`, `SUB_TOPICS_TSV` (`run-decompose.sh:20-28`).
- `scripts/local-setup.sh`, `scripts/publish.sh`, `scripts/validate_ledger.sh`, `scripts/merge_ledgers.sh`, `scripts/triage-health.sh`, `scripts/fetch-image.sh`.
- There is no `--help` convention across scripts, no global command parser, and no stable exit-code contract beyond a few cases (e.g. `publish.sh` 2 = nothing to commit, 1 = failure, `publish.sh:53-56`).
- Two user-facing slash commands: `/scout` (local subscription) and `/scout-async` (open an Issue) (`README.md:99-108`; `skills/scout/SKILL.md`; `skills/scout-async/SKILL.md`).

### MCP

**Absent.** No MCP server, no MCP client, no tool registration. "MCP" appears only as a research topic in prompts (`sharpen.md:92`; `FUTURE.md:77`).

### Config

No single config schema. Configuration is spread across:
- GitHub Actions repository **Variables** (`SCOUT_MODEL*`, `ATLAS_REPO_OWNER`, `ATLAS_REPO_NAME`) (`research.yml:43-52`; `docs/OPERATE.md:60-90`).
- Environment variables read directly by scripts (`TOPIC`, `DEPTH`, `FORMAT`, `ATLAS_REPO`, `SCOUT_DIR`, `SCOUT_CONFIG_DIR`, `SCOUT_MODEL*`, timeouts, etc.).
- `profile.yml` — gitignored, read-only mounted into the runner at `/home/runner/.scout/profile.yml` (`profile.example.yml`; `docker-compose.yml:17`).
- Atlas `_config.yml` — skeleton/palette/card; `_data/series.yml` for series (`INSTALL.md:37-80`; `scripts/add-to-series.sh`).
- `docker/.env` — runner URL/token, Atlas repo (`docker-compose.yml:7-12`; `INSTALL.md:120-140`).
- Prompt files under `skills/` are configuration in practice, but not validated or versioned as config.

No config parser, schema, validation command, or hot reload. Unknown env vars are silently ignored; missing required vars fail via `: "${VAR:?}"` in some scripts but not all.

### Auth

- **GitHub:** `GITHUB_TOKEN` from Actions, permissions `issues: write`, `contents: read` (`research.yml:35-37`). Author-association gate `OWNER`/`MEMBER` on issue/comment triggers (`research.yml:59,104,165,211,247-250`). Local flow uses `gh`'s stored credentials (`local-issue.sh:1-70`).
- **Atlas write:** SSH deploy key on a Docker volume, host alias `github.com-atlas`, write access (`INSTALL.md:100-140`; `docker-compose.yml:16`).
- **Claude:** subscription login or API key stored in the `claude-auth` volume (`README.md:60-78`; `docker-compose.yml:14`).
- No user accounts, sessions, API tokens for a service, OAuth scopes per user, or tenant isolation.

## 8. Safety

### Prompt injection from fetched content

**Not defended.** The README is explicit: Claude runs with `--dangerously-skip-permissions` in a container with unrestricted network access; "**It is not sandboxed.** A malicious page reached during research could, in principle, instruct Claude to misuse credentials on the runner or scan your LAN" (`README.md:163`). Fetched HTML is passed to the model without delimiters, sanitization, trust labels, or an instruction/data boundary. The only mitigation is operator judgment about which topics to run (`README.md:172`).

### SSRF

**No SSRF protection.** `WebFetch` and `curl` in `fetch-image.sh` accept arbitrary URLs, including `http://127.0.0.1`, RFC1918, link-local, and cloud metadata endpoints. There is no scheme allowlist, no private-IP block, no redirect validation, no DNS rebinding protection, and no egress proxy. `fetch-image.sh` adds only a 10s timeout and a 2KB minimum / 1600px resize (`fetch-image.sh:12-16,70-92`). The Docker container has unrestricted network access (`README.md:163`), so SSRF is equivalent to full LAN access.

### Secret handling

Positive patterns:
- The research agent's environment is stripped of `GH_TOKEN`, `GITHUB_TOKEN`, and `GH_REPO` before `claude` runs; `run.sh` keeps them for publish (`run.sh:146-156`). This prevents the agent from posting its own issue comments.
- GitHub token is scoped to `issues:write` + `contents:read` on Scout only, not Atlas (`README.md:167`; `research.yml:35-37`).
- `.env` is gitignored and documented `chmod 600` (`README.md:157`; `INSTALL.md:120-140`).
- `profile.yml` is gitignored (`gitignore:11`; `profile.example.yml:1-9`).
- The Claude result JSON is written outside the agent-writable tree and copied in after the run (`run.sh:136-164`), and `manifest.json` is re-emitted after agent phases (`run-decompose.sh:548-553`). These are integrity protections against an over-eager agent, not against a malicious page.

Gaps:
- The Atlas SSH key is mounted at `/home/runner/.ssh` and is readable by the same user running the agent (`docker-compose.yml:16`; `Dockerfile:27-38`). An agent with Bash can read it and push anywhere the key allows.
- No secret scanning of model output before publish; no output validator for `<script>`, off-topic URLs, or secret-like strings. `FUTURE.md:78` lists this as future work.
- No read-only root, no capability drop, no seccomp/AppArmor, no egress allowlist. `FUTURE.md:78-90` is an explicit security TODO, and `README.md:172` says these are roadmap.
- Container runs as non-root `runner` after entrypoint chown (`docker/entrypoint.sh:1-12`; `Dockerfile:27-38`), which is good, but it still has full network and a writable home.

### Sandboxing

**None beyond Docker's default namespace isolation.** No per-tool sandbox, no syscall filter, no browser sandbox configuration, no network policy. Playwright/Chromium is installed and used for JS-walled pages, increasing attack surface (`docker/Dockerfile:31-38`; `SKILL.md:196`).

**Bottom line for research-daemon:** treat Scout's safety section as a list of anti-patterns, not a template. The useful patterns to keep are credential stripping from workers, least-privilege tokens, agent-write boundaries, and deterministic publish ownership.

## 9. Maturity

### Git metadata (exact)

```text
$ git rev-list --count HEAD
387
$ git log --reverse --format='%ad %s' --date=short | head -1
2026-04-20 Add Scout + Atlas design spec
$ git log -1 --format='%ad %s' --date=short
2026-07-15 fix AskUserQuestions
$ git tag -l
(no output)
$ git branch -a
* main
  remotes/origin/HEAD -> origin/main
  remotes/origin/main
  remotes/origin/fetch-image
```

- **Commits:** 387 (all reachable; `--all` also 387).
- **First commit:** 2026-04-20 21:59 +0200.
- **Last commit:** 2026-07-15 11:25 +0200.
- **Releases/tags:** none. Plugin manifest says version `1.1.1` (` .claude-plugin/plugin.json:4`) but there is no git tag or GitHub release artifact.
- **Tracked files:** 162 (`git ls-files | wc -l`); repository ~19 MB including `.git`.
- **Authors:** 209 `Laoujin <woutervs@hotmail.com>`, 165 `Claude Code <wouter.van.schandevijl@itenium.be>`, 11 `Wouter Van Schandevijl <...fluxys.com>`, 2 `Wouter Van Schandevijl <woutervs@hotmail.com>`. This is effectively a **single-maintainer project with heavy Claude Code co-authorship**.
- **Activity:** bursts in Apr-May 2026, a gap late May, activity through 2026-07-15, then **no commits for ~3.5 months** as of 2026-11-02. No release cadence.

### Tests

- **45 test scripts** under `tests/` (`ls tests/test_*.sh | wc -l`), plain Bash harnesses, hermetic temp dirs, no bats/pytest. Fixtures for ledgers, frontmatter, sharpen snapshots, publish failures (`tests/fixtures/`).
- I ran all 45 in this checkout: **38 passed, 7 failed**.

```text
PASS=38 FAIL=7
FAIL tests/test_backfill_metadata.sh
FAIL tests/test_inject_cost.sh
FAIL tests/test_inject_cover.sh
FAIL tests/test_scout_checkbox_gates.sh
FAIL tests/test_triage_scan.sh
FAIL tests/test_validate_frontmatter.sh
FAIL tests/test_validate_ledger.sh
```

Failure causes:
- `test_inject_cost`, `test_inject_cover`, `test_validate_ledger`: scripts are tracked as mode `100644` and the tests execute them directly, yielding `Permission denied` (rc=126). `git ls-files -s scripts` shows 32 files at 100644 and only 1 executable. Production often invokes `bash script`, masking this.
- `test_backfill_metadata`, `test_triage_scan`, `test_validate_frontmatter`: require `python3` (and PyYAML for auto-fix); `python3` is not installed in this survey environment. These are environment-dependent rather than necessarily repo bugs.
- `test_scout_checkbox_gates`: stale assertions against the current `skills/scout/SKILL.md` (e.g. expects 4 AskUserQuestion headers, finds 6; expects strings that are no longer present). This is a real repo/test drift.

**No CI test workflow.** The only workflows are `research.yml`, `triage.yml`, and `pages.yml`; none runs `tests/`. Tests are manual and can rot silently (as the stale gate test shows).

### Docs quality

**High for a personal project.** README covers purpose, install, usage, depth tiers, operations, troubleshooting, security, and alternatives (`README.md`). `INSTALL.md` is a complete manual install. `docs/OPERATE.md` covers upgrades, auth, runner-token rotation, model cost, and triage. `FUTURE.md` lays out a corpus/capture/active roadmap and security TODO. `COMPARISON.md` positions it against other deep-research projects. The `docs/superpowers/specs/` and `plans/` contain three detailed design/implementation records (provenance metadata, local HTML views, worktree isolation), which are unusually good prior art for design rationale.

### Hygiene signals

- A Python bytecode cache is tracked: `skills/scout-triage/__pycache__/scan.cpython-312.pyc` (`git ls-files`). Not gitignored.
- Script exec bits are mostly absent (32/33 at `100644`).
- No `TODO.md` despite `FUTURE.md:78` referencing it; no `CHANGELOG.md`, `CONTRIBUTING.md`, or `SECURITY.md`.
- No issue/PR templates beyond the research Issue Form; no release automation.
- `DUMMY_RESEARCH.md` is a smoke-test prompt for the decompose pipeline, useful for cheap end-to-end testing.

### Abandoned / incomplete areas

- **Security hardening** (egress allowlist, seccomp, read-only root, output validator) is explicitly roadmap, not implemented (`FUTURE.md:73-90`; `README.md:172`).
- **Provider abstraction / MCP search backend / claim-level quote verification** are named as future "rigor bolt-ons" (`FUTURE.md:77`).
- **Parallel CI decomposition** is claimed but not implemented (`README.md:86` vs `run-decompose.sh:234-309`).
- **Test CI and release engineering** are absent.
- **Budget caps, cancellation, retries, cost dashboard** are future work (`FUTURE.md:80`).
- The project is not dead — it has a coherent roadmap and recent design docs — but it is a **personal tool at 1.x**, not a maintained platform.

## 10. Fit for `research-daemon`

### What can be reused directly

**No Rust code.** There is nothing to `cargo add` or vendor. The reusable assets are **design artifacts and data contracts**:

1. **Citation ledger schema and merge semantics** — `skills/scout-research/SKILL.md:137-141`, `scripts/merge_ledgers.sh:19-30`, `scripts/validate_ledger.sh:17-71`.
   - Port the JSONL schema to a Rust `Evidence` struct: `n`, `url`, `claim`, `source_type`, `quote`, `github_stars`.
   - Keep first-seen URL dedupe + stable renumbering; keep "every marker resolves" validation.
   - Add what Scout lacks: server-side quote verification, content hash, fetched-at timestamp, HTTP status, retrieval method.
2. **Depth tiers and per-tier budgets** — `README.md:125-131`, `scripts/lib-models.sh:17-35`, `SKILL.md:90-97`.
   - Map `recon`/`survey`/`expedition` to query budget, researcher count, citation target, review depth, and model tier. This is directly portable as a Rust enum + config table.
3. **Sharpen/decompose prompt structure** — `skills/scout-research/sharpen.md:38-107`.
   - Verb-led deliverable, explicit Scope/Compare/Constraints/Output, preserve steering hints verbatim, 2-8 sub-topics, completeness sweep as opt-in.
   - In Rust, make this a typed `QueryPlan` with `subquestions`, `constraints`, `coverage_suggestions`, `depth`, and provenance (`raw_topic` vs `sharpened_topic`).
4. **Evidence-first output contract** — `SKILL.md:101-146`, `CLAUDE.md:9-20`.
   - TL;DR first, inline citations, no trailing references, "no URL ⇒ no claim", source-type labels. These translate directly into a report renderer and validator.
5. **Reviewer delta protocol** — `.claude/agents/scout-reviewer.md:19-56`.
   - Blocking categories (citation–claim mismatch, orphan claim, broken numbering, empty ledger fields), coverage gaps, prose nits. In Rust, implement the deterministic subset and use an LLM only for claim/quote entailment.
6. **Status taxonomy and resumability** — `scripts/run-decompose.sh:138-175,242-309`.
   - `success`, `error_with_content`, `failed`, `skipped_success`, `skipped_soft_timeout`, `failed_hard_timeout`. Persist per-child status and skip successful children on rerun.
   - Replace directory sniffing with a real `job`/`attempt` table and idempotency keys.
7. **Deterministic metadata stamping** — `scripts/run.sh:242-258`, `scripts/inject_cost.sh`, `scripts/inject-run-metadata.sh`.
   - Never trust the agent to report cost/duration/model; capture from the execution result and inject after the fact. This is a strong pattern for a daemon.
8. **Per-run worktree isolation and publish race handling** — `scripts/local-setup.sh:25-47`, `scripts/lib-publish.sh:92-131`.
   - Unique run directory, atomic create, rebase-retry, branch fallback. In Rust, use a per-job temp/object-store prefix and optimistic concurrency on the artifact store.
9. **Artifact health scan categories** — `skills/scout-triage/scan.py:1-80` and `SKILL.md`.
   - Port `DEAD`, `GENUINE_FAILURE`, `FALSE_FLAG`, `MANIFEST_MISMATCH`, `LEDGER_MISMATCH`, `MISSING_*` into a `research-daemon doctor` command.
10. **Failure classification** — `scripts/run.sh:36-55` maps usage/rate-limit/quota/overloaded/429/529 to actionable messages. A daemon should classify provider errors similarly and decide retry vs fail.

### What is worth borrowing as a design idea but not code

- **Human-in-the-loop sharpen gate** (`scripts/issue-comment.sh:60-95`; `.github/workflows/research.yml:161-172`). Good for cost control and intent alignment; in a daemon, expose as a `plan.review` API/CLI step rather than an Issue checkbox.
- **Completeness sweep** (`sharpen.md:86-92`). Good prompt design; implement as a plan post-processor that suggests missing facets without silently adding them.
- **Compressed researcher handoff** (`deep.md:67-69`). Keep raw trajectories out of the parent context; store them in the ledger/artifact store, pass summaries only. This is directly relevant to context-window management.
- **One remediation round, one reviewer pass** (`deep.md:42-48`). Hard caps prevent runaway loops; implement as a state machine with attempt counters.
- **Honest synthesis of failed angles** (`synthesis.md:23-33`). Do not hide missing sub-questions; surface them in the artifact and API response.
- **Non-blocking soft-fail log** (`run.sh:29-32,224-258`). Separate "research succeeded but metadata/ledger validation failed" from "research failed"; a daemon should model this as warnings on a successful job.
- **Agent write boundary** (`SKILL.md:12`). The agent writes only into a designated directory; the orchestrator owns publish. In Rust, enforce with a `Workspace` handle and no git/network credentials in the worker.
- **Model tiering** (`lib-models.sh`). Use a cheap model for classification/candidacy, a base model for research/synthesis, a deep model for contested synthesis. For a Tavily-backed daemon, analogously tier *query budgets* and *synthesis depth*.

### What to avoid

1. **Do not use an LLM as the search transport.** Scout's search is opaque, non-reproducible, unrate-limited, and impossible to audit. `research-daemon` should own the provider calls (Tavily/Kagi/etc.), normalize results, and record raw responses.
2. **Do not rely on agent-written ledgers without verification.** Scout's ledger is written by the model; `validate_ledger.sh` checks shape, not truth. A malicious or hallucinating model can fabricate `quote` and `claim`. Fetch and extract server-side, hash the content, and verify quotes against stored text.
3. **Do not copy the GitHub Actions job model.** One 270-minute job per run, no queue, no cancellation, no retry, no budget enforcement, and a 4h20m hard timeout are not a daemon. Use a real scheduler with per-job state, leases, heartbeats, and cancellation.
4. **Do not copy the sequential decompose loop** (`run-decompose.sh:234-309`) if you need parallel querying. It contradicts the README and leaves the main concurrency win on the table.
5. **Do not copy the security posture.** `--dangerously-skip-permissions`, unrestricted egress, no SSRF checks, no prompt-injection boundary, and a readable SSH key in the worker are unacceptable for a daemon that fetches arbitrary web content. See §8.
6. **Do not copy the file-as-database / frontmatter-as-schema approach.** Awk-based YAML mutation, agents clobbering files, re-emitting manifests to repair damage, and directory sniffing for resume are all symptoms of missing durable state. Use SQLite/Postgres for jobs/evidence, and write artifacts as immutable blobs plus a JSON export.
7. **Do not copy the bash string parsing of issues/sub-topics** (`lib-issue-parse.sh`) into a service API. It is test-covered but brittle; define typed request/response schemas.
8. **Do not copy the dual md/html/view authoring pipeline** into the core. It is product-specific presentation logic; keep it as a renderer plug-in, not a research phase.
9. **Do not copy the "agent owns metadata" or "agent owns publish" ambiguity.** Scout fixed this with deterministic stamping and credential stripping; keep those fixes, but make the boundary structural.

### Rough porting effort

Assuming a Rust workspace with an API/CLI/MCP surface and a daemon:

| Area | Effort | Notes |
|---|---:|---|
| Evidence ledger + validation + merge | 3-5 days | Small, direct port; add DB schema and quote verification. |
| Depth/tier config + budget model | 2-4 days | Enum + config; easy. |
| Query plan / sharpen / decompose prompts | 1-2 weeks | Mostly prompt/spec translation; needs typed plan and validation. |
| Provider layer (Tavily/Kagi) + rate limiting + retries + cache | 2-4 weeks | Must be built from scratch; Scout offers no code. |
| Job scheduler / daemon / leases / cancellation | 3-5 weeks | Must be built from scratch. |
| Fetch/extract/normalize + SSRF/prompt-injection defenses | 2-4 weeks | Must be built from scratch; security-critical. |
| Synthesis/report renderer + citations | 1-2 weeks | Contract is clear; rendering is straightforward. |
| Artifact store + resumability + doctor/triage | 1-2 weeks | Borrow status taxonomy and scan categories. |
| API/CLI/MCP surfaces | 2-3 weeks | No prior art; design fresh. |
| End-to-end tests / fixtures | 1-2 weeks | Scout's Bash fixtures are a useful checklist, not reusable code. |

**Overall:** roughly **3-4 months for one experienced Rust engineer** to reach a credible MVP with the same research quality, or **6-10 weeks** if the provider layer and daemon are simplified. Scout contributes **design leverage, not implementation leverage**.

## 11. Verdict

**Mine for parts — reference the workflow/specs, not the code.** Scout has no Rust, no daemon, no API/MCP, no database, and no search-provider abstraction, so it cannot be adopted as a base. Its durable value is a set of hard-won contracts: the depth tiers, the sharpen→decompose→review→synthesize pipeline, the JSONL evidence ledger, the status taxonomy, deterministic metadata stamping, and the artifact health categories. Borrow those as specs and port the small deterministic pieces; build the scheduler, provider layer, persistence, and security model from scratch.
