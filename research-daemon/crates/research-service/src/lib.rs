//! Worker loop and orchestration for research jobs.

pub mod credentials;
pub mod error;
pub mod runner;
pub mod worker;

pub use credentials::{resolve_api_key, resolve_llm_api_key, resolve_optional_reddit_api_key};
pub use error::ServiceError;
pub use research_pipeline::{Bundle, RunMetadata};
pub use runner::{RunOutcome, Runner};
pub use worker::{Worker, WorkerOptions};

use std::sync::Arc;

use research_core::{Config, Job, JobId, RedditMode};
use research_llm::{AnyLlm, FakeLlm, OpenAiCompatibleConfig, OpenAiCompatibleProvider};
use research_provider::{
    AnyProvider, AnyReddit, Budget, FakeProvider, FakeRedditProvider, RedditapisProvider,
    TavilyProvider,
};
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

/// Build the optional Reddit provider.
///
/// Returns `None` when Reddit is disabled or no key is configured. A missing
/// key is a normal state, not a startup failure: the agent simply keeps using
/// web search. The deterministic fake is selected by `fake`.
pub async fn build_reddit_provider(
    config: &Config,
    fake: bool,
) -> Result<Option<AnyReddit>, ServiceError> {
    if fake {
        return Ok(Some(AnyReddit::Fake(FakeRedditProvider)));
    }
    if config.provider.reddit.mode == RedditMode::Never {
        return Ok(None);
    }
    if config.provider.reddit.backend != "redditapis" {
        return Err(ServiceError::Message(format!(
            "unsupported reddit backend '{}'; only 'redditapis' is implemented",
            config.provider.reddit.backend
        )));
    }

    let Some(api_key) = resolve_optional_reddit_api_key(config)? else {
        tracing::warn!(
            key = %config.provider.reddit.api_key_env,
            "Reddit is enabled but no key is configured; falling back to web search"
        );
        return Ok(None);
    };

    let cache = Arc::new(SqliteCache::open(config.cache_path()).await?);
    let blobs = Arc::new(BlobStore::new(config.blob_dir()));
    let budget = Arc::new(Budget::new(
        config.budget.max_queries,
        config.budget.max_credits,
    ));
    let provider = RedditapisProvider::new(&config.provider.reddit, api_key, cache, blobs, budget)?;
    Ok(Some(AnyReddit::Redditapis(provider)))
}

/// Build the configured model provider, or the deterministic fake.
///
/// The real provider resolves its key from the environment first and then from
/// SecretSpec, matching the Tavily credential path.
pub fn build_llm_provider(config: &Config, fake: bool) -> Result<AnyLlm, ServiceError> {
    if fake {
        return Ok(AnyLlm::Fake(FakeLlm::agent()));
    }

    let api_key = resolve_llm_api_key(config)?;
    let provider =
        OpenAiCompatibleProvider::new(OpenAiCompatibleConfig::from(&config.llm), api_key)?;
    Ok(AnyLlm::OpenAi(provider))
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
