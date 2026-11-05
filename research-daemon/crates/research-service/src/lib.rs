//! Worker loop and orchestration for research jobs.

pub mod credentials;
pub mod error;
pub mod runner;
pub mod worker;

pub use credentials::resolve_api_key;
pub use error::ServiceError;
pub use research_pipeline::{Bundle, RunMetadata};
pub use runner::{RunOutcome, Runner};
pub use worker::{Worker, WorkerOptions};

use std::sync::Arc;

use research_core::{Config, Job, JobId};
use research_provider::{AnyProvider, Budget, FakeProvider, TavilyProvider};
use research_store::{BlobStore, JobStore, SqliteCache};

/// Build the production Tavily provider from resolved configuration.
pub async fn build_tavily_provider(config: &Config) -> Result<TavilyProvider, ServiceError> {
    let api_key = resolve_api_key(config)?;
    let cache = Arc::new(SqliteCache::open(config.cache_path()).await?);
    let blobs = Arc::new(BlobStore::new(config.blob_dir()));
    let budget = Arc::new(Budget::new(
        config.budget.max_queries,
        config.budget.max_credits,
    ));

    Ok(TavilyProvider::new(
        &config.provider.tavily,
        api_key,
        cache,
        blobs,
        budget,
    )?)
}

/// Select the real provider or the deterministic fake.
pub async fn build_provider(config: &Config, fake: bool) -> Result<AnyProvider, ServiceError> {
    if fake {
        Ok(AnyProvider::Fake(FakeProvider))
    } else {
        Ok(AnyProvider::Tavily(build_tavily_provider(config).await?))
    }
}

/// Record bundle approval, then requeue a plan awaiting review.
pub async fn approve_plan(store: &JobStore, id: JobId) -> Result<Job, ServiceError> {
    let job = store.require(id).await?;
    if job.status != research_core::JobStatus::AwaitingReview {
        return Err(ServiceError::Message(format!(
            "job {id} is {} and is not awaiting plan review",
            job.status
        )));
    }
    Bundle::open(&job.output_dir)?.mark_plan_approved()?;
    Ok(store.approve_plan(id).await?)
}
