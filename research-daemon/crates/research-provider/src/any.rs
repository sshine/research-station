use async_trait::async_trait;
use research_core::{ExtractRequest, ExtractResponse, SearchRequest, SearchResponse};

use crate::fake::FakeProvider;
use crate::tavily::TavilyProvider;
use crate::{ProviderError, ProviderResponse, SearchProvider};

/// Runtime selection between the real provider and the deterministic fake.
#[derive(Clone)]
pub enum AnyProvider {
    Tavily(TavilyProvider),
    Fake(FakeProvider),
}

impl AnyProvider {
    pub fn is_fake(&self) -> bool {
        matches!(self, Self::Fake(_))
    }
}

#[async_trait]
impl SearchProvider for AnyProvider {
    fn id(&self) -> &'static str {
        match self {
            Self::Tavily(provider) => provider.id(),
            Self::Fake(provider) => provider.id(),
        }
    }

    async fn search(
        &self,
        request: SearchRequest,
    ) -> Result<ProviderResponse<SearchResponse>, ProviderError> {
        match self {
            Self::Tavily(provider) => provider.search(request).await,
            Self::Fake(provider) => provider.search(request).await,
        }
    }

    async fn extract(
        &self,
        request: ExtractRequest,
    ) -> Result<ProviderResponse<ExtractResponse>, ProviderError> {
        match self {
            Self::Tavily(provider) => provider.extract(request).await,
            Self::Fake(provider) => provider.extract(request).await,
        }
    }
}
