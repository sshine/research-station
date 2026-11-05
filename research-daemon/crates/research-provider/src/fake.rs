use async_trait::async_trait;
use research_core::{
    ExtractRequest, ExtractResponse, ExtractResult, SearchRequest, SearchResponse, SearchResult,
    Usage, slugify,
};
use sha2::{Digest, Sha256};

use crate::ProviderResponse;
use crate::SearchProvider;
use crate::error::ProviderError;

/// Deterministic local provider for development, tests, and `--fake` runs.
///
/// It returns stable URLs and prose derived from the query, never touches the
/// network, and costs no credits. The prose is long enough for the extractive
/// synthesizer to produce auditable quotes.
#[derive(Debug, Clone, Default)]
pub struct FakeProvider;

#[async_trait]
impl SearchProvider for FakeProvider {
    fn id(&self) -> &'static str {
        "fake"
    }

    async fn search(
        &self,
        request: SearchRequest,
    ) -> Result<ProviderResponse<SearchResponse>, ProviderError> {
        let query = request.query.trim().to_owned();
        let count = usize::from(request.max_results.unwrap_or(3).clamp(1, 3));
        let slug = slugify(&query, 40);
        // The slug alone collides when two long seeds share a prefix, which
        // would make the collector discard the second seed's results.
        let suffix = hex::encode(Sha256::digest(query.as_bytes()));
        let suffix = &suffix[..8];

        let results = (1..=count)
            .map(|index| {
                let content = fake_content(&query);
                SearchResult {
                    title: format!("{query} — fake source {index}"),
                    url: format!("https://example{index}.test/{slug}-{suffix}"),
                    content: content.clone(),
                    score: Some(0.9 - (index as f64 * 0.05)),
                    raw_content: Some(content),
                    published_date: Some("2026-10-01".to_owned()),
                    id: Some(format!("fake-{index}")),
                }
            })
            .collect();

        Ok(ProviderResponse {
            provider: "fake".to_owned(),
            data: SearchResponse {
                query,
                answer: None,
                results,
                response_time: Some(0.001),
                request_id: Some("fake".to_owned()),
                usage: Some(Usage { credits: Some(1) }),
            },
            cache_hit: false,
            raw_hash: None,
            credits: Some(1),
            request_id: Some("fake".to_owned()),
            latency_ms: 1,
        })
    }

    async fn extract(
        &self,
        request: ExtractRequest,
    ) -> Result<ProviderResponse<ExtractResponse>, ProviderError> {
        let results = request
            .urls
            .into_iter()
            .map(|url| ExtractResult {
                raw_content: fake_content(&url),
                url,
            })
            .collect();

        Ok(ProviderResponse {
            provider: "fake".to_owned(),
            data: ExtractResponse {
                results,
                failed_results: Vec::new(),
                response_time: Some(0.001),
                request_id: Some("fake".to_owned()),
                usage: Some(Usage { credits: Some(1) }),
            },
            cache_hit: false,
            raw_hash: None,
            credits: Some(1),
            request_id: Some("fake".to_owned()),
            latency_ms: 1,
        })
    }
}

fn fake_content(topic: &str) -> String {
    format!(
        "Investigating {topic} shows that the subject is documented in a deterministic fake source. \
         Independent practitioners report that {topic} has measurable trade-offs, including cost \
         and latency differences across deployments. One concrete limitation is that results depend \
         heavily on the quality of the query and the independence of the sources. Recent comparisons \
         show that {topic} works best when evidence is evaluated source by source rather than \
         averaged into a single summary."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_search_is_deterministic_and_quotable() {
        let provider = FakeProvider;
        let response = provider
            .search(SearchRequest::new("Tavily pricing"))
            .await
            .unwrap();

        assert_eq!(response.provider, "fake");
        assert_eq!(response.data.results.len(), 3);
        assert!(response.data.results.iter().all(|result| {
            result
                .raw_content
                .as_deref()
                .is_some_and(|content| content.len() > 100)
        }));
    }
}
