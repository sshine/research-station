//! Provider layer: normalized search/extract, rate limiting, budgets, caching,
//! retries, and raw-response persistence.

pub mod any;
pub mod budget;
pub mod error;
pub mod fake;
pub mod limiter;
pub mod tavily;

pub use any::AnyProvider;
pub use budget::{Budget, BudgetExceeded, QueryReservation};
pub use error::ProviderError;
pub use fake::FakeProvider;
pub use limiter::{RateLimitPermit, RateLimiter};
pub use tavily::TavilyProvider;

use async_trait::async_trait;
use research_core::{ExtractRequest, ExtractResponse, SearchRequest, SearchResponse};
use serde::Serialize;

/// A provider response plus the operational metadata needed for an evidence
/// ledger.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderResponse<T> {
    pub provider: String,
    pub data: T,
    pub cache_hit: bool,
    pub raw_hash: Option<String>,
    pub credits: Option<u64>,
    pub request_id: Option<String>,
    pub latency_ms: u64,
}

/// Every search backend implements this trait. The pipeline never calls a
/// vendor HTTP API directly.
#[async_trait]
pub trait SearchProvider: Send + Sync {
    fn id(&self) -> &'static str;

    async fn search(
        &self,
        request: SearchRequest,
    ) -> Result<ProviderResponse<SearchResponse>, ProviderError>;

    async fn extract(
        &self,
        request: ExtractRequest,
    ) -> Result<ProviderResponse<ExtractResponse>, ProviderError>;
}
