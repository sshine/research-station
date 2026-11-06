use std::collections::HashSet;
use std::path::PathBuf;

use chrono::Utc;
use research_core::{Depth, Job, JobStatus, ReviewPolicy, SearchDepth};
use research_pipeline::{
    Bundle, CollectionLimits, Collector, RuleBasedPlanner, RunMetadata, RunUsage, audit,
    render_audit, render_report, synthesize,
};
use research_provider::SearchProvider;
use research_store::JobStore;

use crate::ServiceError;

#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub status: JobStatus,
    pub report_path: PathBuf,
    pub sources: usize,
    pub claims: usize,
    pub awaiting_review: bool,
    pub stop_reason: Option<String>,
    pub rounds: u32,
}

/// Executes one claimed job through planning, collection, synthesis, and audit.
///
/// Every step is idempotent against the bundle on disk. A killed worker leaves
/// `progress.v1.json` and the JSONL ledgers behind; the next claim continues
/// from the last completed subquestion.
pub struct Runner<'a, P: SearchProvider> {
    store: &'a JobStore,
    provider: &'a P,
}

impl<'a, P: SearchProvider> Runner<'a, P> {
    pub fn new(store: &'a JobStore, provider: &'a P) -> Self {
        Self { store, provider }
    }

    pub async fn run(&self, job: Job) -> Result<RunOutcome, ServiceError> {
        let started_at = Utc::now();
        let bundle = Bundle::open(&job.output_dir)?;
        bundle.write_brief(&job)?;

        let plan = match bundle.load_plan()? {
            Some(plan) => plan,
            None => {
                let plan = RuleBasedPlanner::plan(&job.request);
                bundle.save_plan(&plan)?;
                plan
            }
        };

        let mut progress = bundle.load_progress()?;
        if job.request.review == ReviewPolicy::Manual
            && !progress.plan_approved
            && job.status == JobStatus::Planning
        {
            let job = self
                .store
                .transition(job.id, JobStatus::AwaitingReview)
                .await?;
            return Ok(RunOutcome {
                status: job.status,
                report_path: bundle.report_path(),
                sources: 0,
                claims: 0,
                awaiting_review: true,
                stop_reason: Some("awaiting-review".to_owned()),
                rounds: 0,
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

        for subquestion in &plan.subquestions {
            if progress.is_complete(subquestion.id) {
                continue;
            }
            if self.cancellation_requested(job.id).await? {
                return Err(ServiceError::Cancelled(job.id));
            }

            let remaining_queries = (job.request.budget.max_queries as usize)
                .saturating_sub(queries.iter().filter(|query| !query.cache_hit).count());
            let remaining_sources =
                (job.request.budget.max_sources as usize).saturating_sub(sources.len());
            let credits_used: u64 = queries
                .iter()
                .filter(|query| !query.cache_hit)
                .filter_map(|query| query.credits)
                .sum();
            let remaining_credits = job.request.budget.max_credits.saturating_sub(credits_used);

            if remaining_queries == 0 || remaining_sources == 0 || remaining_credits == 0 {
                break;
            }

            let collected = collector
                .collect(
                    subquestion,
                    CollectionLimits {
                        max_queries: remaining_queries,
                        max_sources: remaining_sources,
                        max_credits: remaining_credits,
                    },
                    &mut known_urls,
                    &mut known_hashes,
                )
                .await?;

            // Notes first: a source row must never point at missing content.
            for note in &collected.notes {
                bundle.write_note(note)?;
            }
            for query in collected.queries {
                bundle.append_query(&query)?;
                queries.push(query);
            }
            for source in collected.sources {
                bundle.append_source(&source)?;
                sources.push(source);
            }

            progress.mark_complete(subquestion.id);
            bundle.save_progress(&progress)?;
        }

        self.ensure_status(job.id, JobStatus::Synthesizing).await?;
        let sources = bundle.load_sources()?;
        let contents = bundle.load_contents(&sources)?;
        let synthesis = synthesize(&plan, &sources, &contents);

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
        let metadata = RunMetadata {
            schema_version: 1,
            job_id: job.id.to_string(),
            question: job.request.question.clone(),
            depth: job.request.depth.as_str().to_owned(),
            provider: self.provider.id().to_owned(),
            planner: "rule-based-v1".to_owned(),
            synthesizer: "extractive-v1".to_owned(),
            engine: "deterministic".to_owned(),
            llm_provider: None,
            llm_model: None,
            prompt_version: None,
            agent_rounds: 0,
            stop_reason: None,
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
                llm_tokens: 0,
                llm_cost_usd: None,
            },
            audit: audit.clone(),
        };
        bundle.write_run(&metadata)?;
        bundle.update_index("completed")?;

        let job = self.store.transition(job.id, JobStatus::Completed).await?;
        Ok(RunOutcome {
            status: job.status,
            report_path: bundle.report_path(),
            sources: sources.len(),
            claims: synthesis.claims.len(),
            awaiting_review: false,
            stop_reason: Some("deterministic".to_owned()),
            rounds: 0,
        })
    }

    async fn ensure_status(
        &self,
        id: research_core::JobId,
        next: JobStatus,
    ) -> Result<(), ServiceError> {
        let job = self.store.require(id).await?;
        if job.status == next {
            return Ok(());
        }
        self.store.transition(id, next).await?;
        Ok(())
    }

    async fn cancellation_requested(&self, id: research_core::JobId) -> Result<bool, ServiceError> {
        Ok(self.store.require(id).await?.cancel_requested)
    }
}

fn provider_depth(depth: Depth) -> SearchDepth {
    match depth {
        Depth::Quick => SearchDepth::Basic,
        Depth::Thorough | Depth::Exhaustive => SearchDepth::Advanced,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_maps_to_provider_depth() {
        assert_eq!(provider_depth(Depth::Quick), SearchDepth::Basic);
        assert_eq!(provider_depth(Depth::Thorough), SearchDepth::Advanced);
        assert_eq!(provider_depth(Depth::Exhaustive), SearchDepth::Advanced);
    }
}
