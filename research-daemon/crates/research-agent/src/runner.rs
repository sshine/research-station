use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use chrono::Utc;
use research_core::{
    Claim, ClaimId, Depth, Evidence, Job, JobStatus, PassageId, Plan, PlanConstraints, PlanId,
    QueryRecord, RawContentFormat, RedditSearchMode, RedditSearchRequest, RedditSort, ReviewPolicy,
    SearchDepth, SearchRequest, SearchTopic, SourceId, SourceRecord, Subquestion,
    SubquestionCoverage, SubquestionId, SupportStatus, Synthesis, TimeRange,
};
use research_llm::{LlmProvider, LlmRequest, LlmResponse, LlmUsage};
use research_pipeline::{
    Bundle, CollectionLimits, Collector, PipelineError, RuleBasedPlanner, RunMetadata, RunUsage,
    audit, collect_reddit, render_audit, render_report, synthesize,
};
use research_provider::{AnyReddit, RedditProvider, SearchProvider};
use research_store::{AgentRun, AgentStore, JobStore};
use serde_json::json;

use crate::action::{AgentAction, PlanDraft, SynthesisDraft, parse_json_object};
use crate::prompt::{PROMPT_VERSION, action_request, plan_request, synthesis_request};
use crate::reddit::{
    explicit_reddit_mention, opinion_intent, should_enable_reddit, should_preseed_reddit,
};

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("pipeline error: {0}")]
    Pipeline(#[from] PipelineError),
    #[error("provider error: {0}")]
    Provider(#[from] research_provider::ProviderError),
    #[error("llm error: {0}")]
    Llm(#[from] research_llm::LlmError),
    #[error("store error: {0}")]
    Store(#[from] research_store::StoreError),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Message(String),
}

#[derive(Debug, Clone)]
pub struct AgentOutcome {
    pub status: JobStatus,
    pub report_path: PathBuf,
    pub sources: usize,
    pub claims: usize,
    pub awaiting_review: bool,
    pub stop_reason: Option<String>,
    pub rounds: u32,
    pub llm_usage: LlmUsage,
}

pub struct AgentRunner<'a, P: SearchProvider, L: LlmProvider> {
    store: &'a JobStore,
    provider: &'a P,
    reddit: Option<&'a AnyReddit>,
    llm: &'a L,
    transcripts: &'a AgentStore,
}

impl<'a, P: SearchProvider, L: LlmProvider> AgentRunner<'a, P, L> {
    pub fn new(
        store: &'a JobStore,
        provider: &'a P,
        reddit: Option<&'a AnyReddit>,
        llm: &'a L,
        transcripts: &'a AgentStore,
    ) -> Self {
        Self {
            store,
            provider,
            reddit,
            llm,
            transcripts,
        }
    }

    pub async fn run(&self, job: Job) -> Result<AgentOutcome, AgentError> {
        let (result, stop_reason) =
            match tokio::time::timeout(job.request.budget.wall_time(), self.run_inner(job.clone()))
                .await
            {
                Ok(result) => (result, "error"),
                Err(_) => (
                    Err(AgentError::Message("wall-time budget exceeded".to_owned())),
                    "wall_time",
                ),
            };

        if let Err(err) = &result {
            let _ = self
                .transcripts
                .append(job.id, "run_error", json!({"error": err.to_string()}))
                .await;
            let _ = self
                .transcripts
                .finish_run(
                    job.id,
                    "failed",
                    Some(stop_reason),
                    &json!({"error": err.to_string()}),
                )
                .await;
        }
        result
    }

    async fn run_inner(&self, job: Job) -> Result<AgentOutcome, AgentError> {
        let started_at = Utc::now();
        let bundle = Bundle::open(&job.output_dir)?;
        bundle.write_brief(&job)?;

        let run = AgentRun {
            job_id: job.id,
            engine: "agent-v1".to_owned(),
            provider: self.llm.id().to_owned(),
            model: self.llm.model().to_owned(),
            prompt_version: PROMPT_VERSION.to_owned(),
            started_at,
            finished_at: None,
            status: "running".to_owned(),
            stop_reason: None,
            usage: json!({}),
        };
        self.transcripts.start_run(&run).await?;
        self.transcripts
            .append(
                job.id,
                "run_start",
                json!({
                    "question": job.request.question,
                    "depth": job.request.depth.as_str(),
                    "engine": "agent-v1",
                    "model": self.llm.model(),
                }),
            )
            .await?;

        let mut usage = LlmUsage::default();
        let mut total_rounds = 0u32;
        let mut stop_reasons = Vec::new();

        let plan = match bundle.load_plan()? {
            Some(plan) => plan,
            None => self.make_plan(&job, &mut usage).await?,
        };
        bundle.save_plan(&plan)?;

        let explicit_reddit = explicit_reddit_mention(&job.request.question);
        let opinion_reddit = opinion_intent(&job.request.question);
        let trigger_preseed = explicit_reddit || opinion_reddit;
        let mut reddit_available = self.reddit.is_some()
            && should_enable_reddit(job.request.reddit, &job.request.question, &plan);
        let max_reddit_searches = self
            .reddit
            .map(RedditProvider::max_searches_per_job)
            .unwrap_or(0);
        let mut reddit_searches = 0u32;
        let mut trigger_preseeded = false;
        if reddit_available {
            let reason = if explicit_reddit {
                "explicit"
            } else if opinion_reddit {
                "opinion_intent"
            } else {
                "auto"
            };
            self.transcripts
                .append(
                    job.id,
                    "reddit_enabled",
                    json!({
                        "reason": reason,
                        "max_searches": max_reddit_searches,
                        "provider": self.reddit.map(RedditProvider::id),
                    }),
                )
                .await?;
        }

        let mut progress = bundle.load_progress()?;
        if job.request.review == ReviewPolicy::Manual
            && !progress.plan_approved
            && job.status == JobStatus::Planning
        {
            let job = self
                .store
                .transition(job.id, JobStatus::AwaitingReview)
                .await?;
            let stop_reason = Some("awaiting-review".to_owned());
            self.transcripts
                .append(job.id, "run_pause", json!({"reason": "awaiting-review"}))
                .await?;
            self.transcripts
                .finish_run(
                    job.id,
                    "awaiting-review",
                    stop_reason.as_deref(),
                    &json!({"usage": usage, "rounds": total_rounds}),
                )
                .await?;
            return Ok(AgentOutcome {
                status: job.status,
                report_path: bundle.report_path(),
                sources: 0,
                claims: 0,
                awaiting_review: true,
                stop_reason,
                rounds: total_rounds,
                llm_usage: usage,
            });
        }

        self.ensure_status(job.id, JobStatus::Running).await?;

        let mut queries = bundle.load_queries()?;
        let mut sources = bundle.load_sources()?;
        let mut known_urls: HashSet<String> = sources
            .iter()
            .map(|source| source.canonical_url.clone())
            .collect();
        let mut known_hashes: HashSet<String> = sources
            .iter()
            .map(|source| source.content_hash.clone())
            .collect();
        let collector = Collector::new(
            self.provider,
            &plan.constraints,
            provider_depth(job.request.depth),
        );
        let max_rounds_per_subquestion =
            (job.request.budget.max_agent_rounds / plan.subquestions.len().max(1) as u32).max(1);

        for subquestion in &plan.subquestions {
            if progress.is_complete(subquestion.id) {
                continue;
            }
            if self.cancellation_requested(job.id).await? {
                stop_reasons.push("cancelled".to_owned());
                break;
            }

            let mut history = Vec::new();

            if reddit_available
                && reddit_searches < max_reddit_searches
                && should_preseed_reddit(
                    job.request.reddit,
                    &job.request.question,
                    subquestion,
                    trigger_preseeded,
                )
            {
                if let Some(reddit) = self.reddit {
                    let (remaining_queries, remaining_sources, remaining_credits) =
                        remaining_budget(&job, &queries, &sources);
                    if remaining_queries > 0 && remaining_sources > 0 && remaining_credits > 0 {
                        let mut request = reddit_request_for_subquestion(subquestion);
                        if trigger_preseed && !trigger_preseeded {
                            request.query = job.request.question.clone();
                        }
                        let limits = CollectionLimits {
                            max_queries: remaining_queries,
                            max_sources: remaining_sources,
                            max_credits: remaining_credits,
                        };
                        match collect_reddit(
                            reddit,
                            subquestion,
                            request,
                            limits,
                            &mut known_urls,
                            &mut known_hashes,
                        )
                        .await
                        {
                            Ok(collected) => {
                                let added = record_collected(
                                    &bundle,
                                    &mut queries,
                                    &mut sources,
                                    collected,
                                )?;
                                reddit_searches = reddit_searches.saturating_add(1);
                                if trigger_preseed {
                                    trigger_preseeded = true;
                                }
                                history.push(format!("reddit pre-seed: added {added} source(s)"));
                                self.transcripts
                                    .append(
                                        job.id,
                                        "tool_result",
                                        json!({
                                            "tool": "reddit_search",
                                            "preseed": true,
                                            "sources_added": added,
                                        }),
                                    )
                                    .await?;
                            }
                            Err(err) => {
                                reddit_available = false;
                                history.push(format!("reddit pre-seed failed: {err}"));
                                self.transcripts
                                    .append(
                                        job.id,
                                        "reddit_fallback",
                                        json!({"reason": err.to_string()}),
                                    )
                                    .await?;
                            }
                        }
                    }
                }
            }

            if reddit_searches >= max_reddit_searches {
                reddit_available = false;
            }

            let mut rounds = 0u32;
            loop {
                if self.cancellation_requested(job.id).await? {
                    stop_reasons.push("cancelled".to_owned());
                    break;
                }

                let (remaining_queries, remaining_sources, remaining_credits) =
                    remaining_budget(&job, &queries, &sources);
                let remaining_tokens = job
                    .request
                    .budget
                    .max_llm_tokens
                    .saturating_sub(usage.total_tokens);

                if remaining_queries == 0 {
                    stop_reasons.push("budget_queries".to_owned());
                    break;
                }
                if remaining_sources == 0 {
                    stop_reasons.push("budget_sources".to_owned());
                    break;
                }
                if remaining_credits == 0 {
                    stop_reasons.push("budget_credits".to_owned());
                    break;
                }
                if remaining_tokens == 0 {
                    stop_reasons.push("budget_llm_tokens".to_owned());
                    break;
                }
                if rounds >= max_rounds_per_subquestion {
                    stop_reasons.push("max_rounds".to_owned());
                    break;
                }

                let sub_sources: Vec<SourceRecord> = sources
                    .iter()
                    .filter(|source| source.subquestion_id == subquestion.id)
                    .cloned()
                    .collect();
                let reddit_available_now =
                    reddit_available && reddit_searches < max_reddit_searches;
                let request = action_request(
                    &job,
                    subquestion,
                    &sub_sources,
                    remaining_queries,
                    remaining_sources,
                    remaining_credits,
                    remaining_tokens,
                    rounds + 1,
                    reddit_available_now,
                    max_reddit_searches.saturating_sub(reddit_searches),
                    &history,
                );
                let response = self
                    .complete_json(job.id, "action", request, &mut usage)
                    .await?;
                let action: AgentAction = match parse_json_object(&response.content) {
                    Ok(action) => action,
                    Err(err) => {
                        history.push(format!("invalid action JSON: {err}"));
                        rounds += 1;
                        total_rounds += 1;
                        continue;
                    }
                };
                self.transcripts
                    .append(
                        job.id,
                        "action",
                        json!({
                            "round": rounds + 1,
                            "subquestion": subquestion.question,
                            "action": action,
                        }),
                    )
                    .await?;

                match action {
                    AgentAction::Search {
                        query,
                        max_results,
                        topic,
                        time_range,
                    } => {
                        let mut request = SearchRequest::new(query.clone())
                            .with_depth(provider_depth(job.request.depth))
                            .with_max_results(
                                max_results
                                    .unwrap_or_else(|| default_max_results(job.request.depth))
                                    .clamp(1, 20),
                            );
                        request.topic = topic.unwrap_or(SearchTopic::General);
                        request.time_range = time_range;
                        request.include_domains = job.request.include_domains.clone();
                        request.exclude_domains = job.request.exclude_domains.clone();
                        request.include_raw_content = Some(RawContentFormat::Markdown);
                        request.include_published_date = true;

                        let limits = CollectionLimits {
                            max_queries: remaining_queries,
                            max_sources: remaining_sources,
                            max_credits: remaining_credits,
                        };
                        let collected = collector
                            .collect_query(
                                subquestion,
                                request,
                                limits,
                                &mut known_urls,
                                &mut known_hashes,
                            )
                            .await?;
                        let added =
                            record_collected(&bundle, &mut queries, &mut sources, collected)?;
                        history.push(format!("search \"{query}\": added {added} source(s)"));
                        self.transcripts
                            .append(
                                job.id,
                                "tool_result",
                                json!({
                                    "tool": "search",
                                    "query": query,
                                    "sources_added": added,
                                }),
                            )
                            .await?;
                    }
                    AgentAction::Extract { urls, query } => {
                        let limits = CollectionLimits {
                            max_queries: remaining_queries,
                            max_sources: remaining_sources,
                            max_credits: remaining_credits,
                        };
                        let collected = collector
                            .collect_extract(
                                subquestion,
                                urls.clone(),
                                query.clone(),
                                limits,
                                &mut known_urls,
                                &mut known_hashes,
                            )
                            .await?;
                        let added =
                            record_collected(&bundle, &mut queries, &mut sources, collected)?;
                        history.push(format!(
                            "extract {} URL(s): added {added} source(s)",
                            urls.len()
                        ));
                        self.transcripts
                            .append(
                                job.id,
                                "tool_result",
                                json!({
                                    "tool": "extract",
                                    "urls": urls,
                                    "query": query,
                                    "sources_added": added,
                                }),
                            )
                            .await?;
                    }
                    AgentAction::RedditSearch {
                        query,
                        subreddit,
                        sort,
                        mode,
                        time_range,
                        max_results,
                        min_score,
                        include_comments,
                        comment_limit,
                    } => {
                        let Some(reddit) = self.reddit else {
                            history.push("reddit_search rejected: provider unavailable".to_owned());
                            self.transcripts
                                .append(
                                    job.id,
                                    "tool_result",
                                    json!({"tool": "reddit_search", "error": "unavailable"}),
                                )
                                .await?;
                            rounds += 1;
                            total_rounds += 1;
                            continue;
                        };
                        if !reddit_available || reddit_searches >= max_reddit_searches {
                            history.push(
                                "reddit_search rejected: per-job search cap reached".to_owned(),
                            );
                            self.transcripts
                                .append(
                                    job.id,
                                    "tool_result",
                                    json!({"tool": "reddit_search", "error": "cap_reached"}),
                                )
                                .await?;
                            rounds += 1;
                            total_rounds += 1;
                            continue;
                        }

                        let mut request = RedditSearchRequest::new(query.clone());
                        request.subreddit = subreddit.clone();
                        request.sort = sort.unwrap_or(RedditSort::Relevance);
                        request.mode = mode.unwrap_or(RedditSearchMode::Posts);
                        request.time_range = time_range;
                        request.max_results = Some(max_results.unwrap_or(25).clamp(1, 100));
                        request.min_score = min_score;
                        request.include_comments = Some(include_comments.unwrap_or(true));
                        request.comment_limit = comment_limit;

                        let limits = CollectionLimits {
                            max_queries: remaining_queries,
                            max_sources: remaining_sources,
                            max_credits: remaining_credits,
                        };
                        match collect_reddit(
                            reddit,
                            subquestion,
                            request,
                            limits,
                            &mut known_urls,
                            &mut known_hashes,
                        )
                        .await
                        {
                            Ok(collected) => {
                                let added = record_collected(
                                    &bundle,
                                    &mut queries,
                                    &mut sources,
                                    collected,
                                )?;
                                reddit_searches = reddit_searches.saturating_add(1);
                                history.push(format!(
                                    "reddit_search \"{query}\": added {added} source(s)"
                                ));
                                self.transcripts
                                    .append(
                                        job.id,
                                        "tool_result",
                                        json!({
                                            "tool": "reddit_search",
                                            "query": query,
                                            "subreddit": subreddit,
                                            "mode": mode
                                                .unwrap_or(RedditSearchMode::Posts)
                                                .as_str(),
                                            "sources_added": added,
                                        }),
                                    )
                                    .await?;
                            }
                            Err(err) => {
                                // A failed Reddit call should degrade the job, not
                                // abort it. Disable Reddit for the rest of this job
                                // and let the model continue with web search.
                                reddit_available = false;
                                history.push(format!("reddit_search failed: {err}"));
                                self.transcripts
                                    .append(
                                        job.id,
                                        "reddit_fallback",
                                        json!({
                                            "reason": err.to_string(),
                                            "query": query,
                                        }),
                                    )
                                    .await?;
                            }
                        }
                    }
                    AgentAction::ReadSource { source_id } => {
                        let id: SourceId = source_id.parse().map_err(|err: uuid::Error| {
                            AgentError::Message(format!("invalid source id: {err}"))
                        })?;
                        let Some(source) = sources.iter().find(|source| source.id == id) else {
                            history.push(format!("read_source {id}: source not found"));
                            self.transcripts
                                .append(
                                    job.id,
                                    "tool_result",
                                    json!({"tool": "read_source", "source_id": id, "error": "not found"}),
                                )
                                .await?;
                            rounds += 1;
                            total_rounds += 1;
                            continue;
                        };
                        let content = bundle.source_content(source)?;
                        let snippet = truncate(&content, 4_000);
                        history.push(format!("read_source {}:\n{}", id, snippet));
                        self.transcripts
                            .append(
                                job.id,
                                "tool_result",
                                json!({
                                    "tool": "read_source",
                                    "source_id": id,
                                    "content": snippet,
                                }),
                            )
                            .await?;
                    }
                    AgentAction::Finish { satisfied, reason } => {
                        let has_source = sources
                            .iter()
                            .any(|source| source.subquestion_id == subquestion.id);
                        self.transcripts
                            .append(
                                job.id,
                                "finish_attempt",
                                json!({
                                    "satisfied": satisfied,
                                    "has_source": has_source,
                                    "reason": reason,
                                }),
                            )
                            .await?;
                        if satisfied && has_source {
                            history.push(format!("finish accepted: {reason}"));
                            break;
                        }
                        history.push(format!(
                            "finish rejected: satisfied={satisfied}, has_source={has_source}, reason={reason}"
                        ));
                    }
                }

                rounds += 1;
                total_rounds += 1;
                if usage.total_tokens >= job.request.budget.max_llm_tokens {
                    stop_reasons.push("budget_llm_tokens".to_owned());
                    break;
                }
            }

            progress.mark_complete(subquestion.id);
            bundle.save_progress(&progress)?;
        }

        self.ensure_status(job.id, JobStatus::Synthesizing).await?;
        let sources = bundle.load_sources()?;
        let contents = bundle.load_contents(&sources)?;
        let synthesis = self
            .synthesize(&job, &plan, &sources, &contents, &mut usage)
            .await?;

        self.ensure_status(job.id, JobStatus::Auditing).await?;
        let audit = audit(&plan, &synthesis, &contents);
        let report = render_report(&plan, &synthesis, &sources, &audit);
        bundle.write_report(&report)?;
        let audit_markdown = render_audit(&plan, &synthesis, &audit);
        bundle.write_audit(&audit_markdown)?;

        let queries = bundle.load_queries()?;
        let credits = queries
            .iter()
            .filter(|query| !query.cache_hit)
            .filter_map(|query| query.credits)
            .sum();
        let stop_reason = if stop_reasons.is_empty() {
            "satisfied".to_owned()
        } else {
            dedupe(stop_reasons).join(",")
        };
        let metadata = RunMetadata {
            schema_version: 1,
            job_id: job.id.to_string(),
            question: job.request.question.clone(),
            depth: job.request.depth.as_str().to_owned(),
            provider: self.provider.id().to_owned(),
            planner: "agent-v1".to_owned(),
            synthesizer: "agent-v1".to_owned(),
            engine: "agent-v1".to_owned(),
            llm_provider: Some(self.llm.id().to_owned()),
            llm_model: Some(self.llm.model().to_owned()),
            prompt_version: Some(PROMPT_VERSION.to_owned()),
            agent_rounds: total_rounds,
            stop_reason: Some(stop_reason.clone()),
            status: if audit.passed {
                "completed".to_owned()
            } else {
                "completed-with-audit-findings".to_owned()
            },
            started_at,
            finished_at: Utc::now(),
            output_dir: job.output_dir.display().to_string(),
            usage: RunUsage {
                queries: queries.len(),
                sources: sources.len(),
                claims: synthesis.claims.len(),
                credits,
                llm_tokens: usage.total_tokens,
                llm_cost_usd: usage.cost_usd,
            },
            audit: audit.clone(),
        };
        bundle.write_run(&metadata)?;
        bundle.update_index("completed")?;

        self.transcripts
            .append(
                job.id,
                "run_end",
                json!({
                    "stop_reason": stop_reason,
                    "rounds": total_rounds,
                    "usage": usage,
                    "audit_passed": audit.passed,
                }),
            )
            .await?;
        self.transcripts
            .finish_run(
                job.id,
                "completed",
                Some(&stop_reason),
                &json!({"usage": usage, "rounds": total_rounds}),
            )
            .await?;

        let job = self.store.transition(job.id, JobStatus::Completed).await?;
        Ok(AgentOutcome {
            status: job.status,
            report_path: bundle.report_path(),
            sources: sources.len(),
            claims: synthesis.claims.len(),
            awaiting_review: false,
            stop_reason: Some(stop_reason),
            rounds: total_rounds,
            llm_usage: usage,
        })
    }

    async fn make_plan(&self, job: &Job, usage: &mut LlmUsage) -> Result<Plan, AgentError> {
        let request = plan_request(job);
        let response = self.complete_json(job.id, "plan", request, usage).await?;
        let draft: PlanDraft = match parse_json_object::<PlanDraft>(&response.content) {
            Ok(draft) if !draft.subquestions.is_empty() => draft,
            Ok(_) => {
                self.transcripts
                    .append(
                        job.id,
                        "plan_fallback",
                        json!({"reason": "model returned no subquestions"}),
                    )
                    .await?;
                return Ok(RuleBasedPlanner::plan(&job.request));
            }
            Err(err) => {
                self.transcripts
                    .append(
                        job.id,
                        "plan_fallback",
                        json!({"reason": format!("invalid plan JSON: {err}")}),
                    )
                    .await?;
                return Ok(RuleBasedPlanner::plan(&job.request));
            }
        };

        Ok(plan_from_draft(job, draft))
    }

    async fn synthesize(
        &self,
        job: &Job,
        plan: &Plan,
        sources: &[SourceRecord],
        contents: &HashMap<SourceId, String>,
        usage: &mut LlmUsage,
    ) -> Result<Synthesis, AgentError> {
        let request = synthesis_request(plan, sources, contents);
        let response = self
            .complete_json(job.id, "synthesis", request, usage)
            .await?;
        let draft: SynthesisDraft = match parse_json_object(&response.content) {
            Ok(draft) => draft,
            Err(err) => {
                self.transcripts
                    .append(
                        job.id,
                        "synthesis_fallback",
                        json!({"reason": format!("invalid synthesis JSON: {err}")}),
                    )
                    .await?;
                return Ok(synthesize(plan, sources, contents));
            }
        };
        let synthesis = synthesis_from_draft(plan, sources, contents, draft);
        if synthesis.claims.is_empty() {
            self.transcripts
                .append(
                    job.id,
                    "synthesis_fallback",
                    json!({"reason": "model produced no auditable claims"}),
                )
                .await?;
            Ok(synthesize(plan, sources, contents))
        } else {
            Ok(synthesis)
        }
    }

    async fn complete_json(
        &self,
        job_id: research_core::JobId,
        kind: &str,
        request: LlmRequest,
        usage: &mut LlmUsage,
    ) -> Result<LlmResponse, AgentError> {
        self.transcripts
            .append(
                job_id,
                format!("{kind}_request"),
                json!({
                    "messages": request.messages,
                    "temperature": request.temperature,
                    "max_tokens": request.max_tokens,
                    "response_format": request.response_format,
                }),
            )
            .await?;
        let response = self.llm.complete(request).await?;
        usage.prompt_tokens = usage
            .prompt_tokens
            .saturating_add(response.usage.prompt_tokens);
        usage.completion_tokens = usage
            .completion_tokens
            .saturating_add(response.usage.completion_tokens);
        usage.total_tokens = usage
            .total_tokens
            .saturating_add(response.usage.total_tokens);
        if let Some(cost) = response.usage.cost_usd {
            usage.cost_usd = Some(usage.cost_usd.unwrap_or(0.0) + cost);
        }
        if usage.billing_source.is_none() {
            usage.billing_source = response.usage.billing_source.clone();
        }
        self.transcripts
            .append(
                job_id,
                format!("{kind}_response"),
                json!({
                    "provider": &response.provider,
                    "model": &response.model,
                    "content": &response.content,
                    "reasoning": &response.reasoning,
                    "finish_reason": &response.finish_reason,
                    "usage": &response.usage,
                    "request_id": &response.request_id,
                    "latency_ms": response.latency_ms,
                }),
            )
            .await?;
        Ok(response)
    }

    async fn ensure_status(
        &self,
        id: research_core::JobId,
        next: JobStatus,
    ) -> Result<(), AgentError> {
        let job = self.store.require(id).await?;
        if job.status == next {
            return Ok(());
        }
        self.store.transition(id, next).await?;
        Ok(())
    }

    async fn cancellation_requested(&self, id: research_core::JobId) -> Result<bool, AgentError> {
        Ok(self.store.require(id).await?.cancel_requested)
    }
}

fn reddit_request_for_subquestion(subquestion: &Subquestion) -> RedditSearchRequest {
    let query = subquestion
        .query_seeds
        .first()
        .cloned()
        .unwrap_or_else(|| subquestion.question.clone());
    let mut request = RedditSearchRequest::new(query);
    request.sort = RedditSort::Relevance;
    request.mode = RedditSearchMode::Posts;
    request.time_range = Some(TimeRange::Year);
    request.max_results = Some(25);
    request.include_comments = Some(true);
    request
}

fn plan_from_draft(job: &Job, draft: PlanDraft) -> Plan {
    let subquestions: Vec<Subquestion> = draft
        .subquestions
        .into_iter()
        .take(8)
        .filter(|subquestion| !subquestion.question.trim().is_empty())
        .map(|subquestion| Subquestion {
            id: SubquestionId::new(),
            question: subquestion.question.trim().to_owned(),
            source_class: subquestion.source_class,
            query_seeds: if subquestion.query_seeds.is_empty() {
                vec![subquestion.question.trim().to_owned()]
            } else {
                subquestion
                    .query_seeds
                    .into_iter()
                    .map(|seed| seed.trim().to_owned())
                    .filter(|seed| !seed.is_empty())
                    .collect()
            },
            success_criterion: if subquestion.success_criterion.trim().is_empty() {
                "At least one independent source supports the answer.".to_owned()
            } else {
                subquestion.success_criterion.trim().to_owned()
            },
        })
        .collect();

    if subquestions.is_empty() {
        return RuleBasedPlanner::plan(&job.request);
    }

    let sharpened_topic = draft.sharpened_topic.trim();
    Plan {
        id: PlanId::new(),
        version: 1,
        raw_topic: job.request.question.clone(),
        sharpened_topic: if sharpened_topic.is_empty() {
            job.request.question.trim().to_owned()
        } else {
            sharpened_topic.to_owned()
        },
        subquestions,
        constraints: PlanConstraints {
            include_domains: job.request.include_domains.clone(),
            exclude_domains: job.request.exclude_domains.clone(),
        },
        budget: job.request.budget,
        created_at: Utc::now(),
    }
}

fn synthesis_from_draft(
    plan: &Plan,
    sources: &[SourceRecord],
    contents: &HashMap<SourceId, String>,
    draft: SynthesisDraft,
) -> Synthesis {
    let mut claims = Vec::new();
    for claim in draft.claims {
        let Some(subquestion) = claim
            .subquestion
            .checked_sub(1)
            .and_then(|index| plan.subquestions.get(index))
        else {
            continue;
        };
        let evidence: Vec<Evidence> = claim
            .evidence
            .into_iter()
            .filter_map(|evidence| {
                let source = evidence
                    .source
                    .checked_sub(1)
                    .and_then(|index| sources.get(index))?;
                let content = contents.get(&source.id)?;
                if !content.contains(&evidence.quote) {
                    return None;
                }
                Some(Evidence {
                    source_id: source.id,
                    passage_id: PassageId::new(),
                    quote: evidence.quote,
                })
            })
            .collect();
        if evidence.is_empty() {
            continue;
        }
        claims.push(Claim {
            id: ClaimId::new(),
            subquestion_id: subquestion.id,
            text: claim.text.trim().to_owned(),
            evidence,
            confidence: claim.confidence.unwrap_or(0.6).clamp(0.0, 1.0),
            support: SupportStatus::Supported,
        });
    }

    let coverage = plan
        .subquestions
        .iter()
        .map(|subquestion| SubquestionCoverage {
            subquestion_id: subquestion.id,
            claim_count: claims
                .iter()
                .filter(|claim| claim.subquestion_id == subquestion.id)
                .count(),
            source_count: sources
                .iter()
                .filter(|source| source.subquestion_id == subquestion.id)
                .count(),
        })
        .collect();

    Synthesis { claims, coverage }
}

fn record_collected(
    bundle: &Bundle,
    queries: &mut Vec<QueryRecord>,
    sources: &mut Vec<SourceRecord>,
    collected: research_pipeline::CollectedSubquestion,
) -> Result<usize, AgentError> {
    for note in &collected.notes {
        bundle.write_note(note)?;
    }
    let added = collected.sources.len();
    for query in collected.queries {
        bundle.append_query(&query)?;
        queries.push(query);
    }
    for source in collected.sources {
        bundle.append_source(&source)?;
        sources.push(source);
    }
    Ok(added)
}

fn remaining_budget(
    job: &Job,
    queries: &[QueryRecord],
    sources: &[SourceRecord],
) -> (usize, usize, u64) {
    let queries_used = queries.iter().filter(|query| !query.cache_hit).count();
    let credits_used: u64 = queries
        .iter()
        .filter(|query| !query.cache_hit)
        .filter_map(|query| query.credits)
        .sum();
    (
        (job.request.budget.max_queries as usize).saturating_sub(queries_used),
        (job.request.budget.max_sources as usize).saturating_sub(sources.len()),
        job.request.budget.max_credits.saturating_sub(credits_used),
    )
}

fn provider_depth(depth: Depth) -> SearchDepth {
    match depth {
        Depth::Quick => SearchDepth::Basic,
        Depth::Thorough | Depth::Exhaustive => SearchDepth::Advanced,
    }
}

fn default_max_results(depth: Depth) -> u8 {
    match depth {
        Depth::Quick => 5,
        Depth::Thorough | Depth::Exhaustive => 8,
    }
}

fn truncate(content: &str, limit: usize) -> String {
    let mut chars = content.chars();
    let truncated: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn dedupe(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::SubquestionDraft;
    use research_core::JobBudget;

    #[test]
    fn plan_draft_gets_stable_ids_and_constraints() {
        let job = Job {
            id: research_core::JobId::new(),
            request: research_core::ResearchRequest::new("A question"),
            status: JobStatus::Planning,
            output_dir: "research/test".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            lease_owner: None,
            lease_expires_at: None,
            attempts: 0,
            last_error: None,
            idempotency_key: None,
            cancel_requested: false,
        };
        let plan = plan_from_draft(
            &job,
            PlanDraft {
                sharpened_topic: "sharpened".to_owned(),
                subquestions: vec![SubquestionDraft {
                    question: "q".to_owned(),
                    source_class: research_core::SourceClass::Primary,
                    success_criterion: "one source".to_owned(),
                    query_seeds: vec![],
                }],
            },
        );
        assert_eq!(plan.sharpened_topic, "sharpened");
        assert_eq!(plan.subquestions.len(), 1);
        assert_eq!(plan.subquestions[0].query_seeds, vec!["q"]);
    }

    #[test]
    fn remaining_budget_counts_non_cache_queries() {
        let job = Job {
            id: research_core::JobId::new(),
            request: research_core::ResearchRequest::new("q"),
            status: JobStatus::Running,
            output_dir: "research/test".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            lease_owner: None,
            lease_expires_at: None,
            attempts: 0,
            last_error: None,
            idempotency_key: None,
            cancel_requested: false,
        };
        assert_eq!(
            remaining_budget(&job, &[], &[]),
            (
                job.request.budget.max_queries as usize,
                job.request.budget.max_sources as usize,
                job.request.budget.max_credits
            )
        );
    }

    #[test]
    fn budget_defaults_are_positive() {
        assert!(JobBudget::default().max_agent_rounds > 0);
    }
}
