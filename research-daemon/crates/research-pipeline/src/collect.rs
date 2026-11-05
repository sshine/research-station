use std::collections::HashSet;

use chrono::Utc;
use research_core::{
    PlanConstraints, QueryId, QueryRecord, RawContentFormat, SearchDepth, SearchRequest,
    SearchTopic, SourceClass, SourceId, SourceRecord, Subquestion, TimeRange,
};
use research_provider::SearchProvider;
use sha2::{Digest, Sha256};
use url::Url;

use crate::PipelineError;
use crate::bundle::NoteFile;
use crate::canonical::canonicalize_url;

/// Remaining per-job allowance handed to the collector on each call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionLimits {
    pub max_queries: usize,
    pub max_sources: usize,
    pub max_credits: u64,
}

impl CollectionLimits {
    pub fn unlimited() -> Self {
        Self {
            max_queries: usize::MAX,
            max_sources: usize::MAX,
            max_credits: u64::MAX,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CollectedSubquestion {
    pub queries: Vec<QueryRecord>,
    pub sources: Vec<SourceRecord>,
    pub notes: Vec<NoteFile>,
}

/// Turns one subquestion's query seeds into provider calls and accepted
/// sources. The caller owns deduplication state so sources are never counted
/// twice across subquestions.
pub struct Collector<'a, P: SearchProvider> {
    provider: &'a P,
    constraints: &'a PlanConstraints,
    depth: SearchDepth,
    max_results: u8,
}

impl<'a, P: SearchProvider> Collector<'a, P> {
    pub fn new(provider: &'a P, constraints: &'a PlanConstraints, depth: SearchDepth) -> Self {
        let max_results = match depth {
            SearchDepth::Fast | SearchDepth::UltraFast | SearchDepth::Basic => 5,
            SearchDepth::Advanced => 8,
        };

        Self {
            provider,
            constraints,
            depth,
            max_results,
        }
    }

    pub async fn collect(
        &self,
        subquestion: &Subquestion,
        limits: CollectionLimits,
        known_urls: &mut HashSet<String>,
        known_hashes: &mut HashSet<String>,
    ) -> Result<CollectedSubquestion, PipelineError> {
        let mut collected = CollectedSubquestion::default();
        let mut credits_used = 0u64;

        for seed in subquestion.query_seeds.iter().take(2) {
            if collected.queries.len() >= limits.max_queries
                || collected.sources.len() >= limits.max_sources
            {
                break;
            }
            if limits.max_credits != u64::MAX && credits_used >= limits.max_credits {
                break;
            }

            let request = self.search_request(subquestion, seed);
            let response = self.provider.search(request).await?;
            let credits = response.credits.unwrap_or(0);
            credits_used = credits_used.saturating_add(credits);

            collected.queries.push(QueryRecord {
                id: QueryId::new(),
                subquestion_id: subquestion.id,
                provider: response.provider.clone(),
                text: seed.clone(),
                at: Utc::now(),
                status: "ok".to_owned(),
                result_count: response.data.results.len(),
                latency_ms: response.latency_ms,
                credits: response.credits,
                raw_hash: response.raw_hash.clone(),
                cache_hit: response.cache_hit,
            });

            for result in response.data.results {
                if collected.sources.len() >= limits.max_sources {
                    break;
                }

                let Some(canonical_url) = canonicalize_url(&result.url) else {
                    continue;
                };
                if !known_urls.insert(canonical_url.clone()) {
                    continue;
                }

                let content = result
                    .raw_content
                    .as_deref()
                    .filter(|content| !content.trim().is_empty())
                    .unwrap_or(&result.content)
                    .trim()
                    .to_owned();
                if content.is_empty() {
                    known_urls.remove(&canonical_url);
                    continue;
                }

                let id = SourceId::new();
                let content_hash = hex::encode(Sha256::digest(content.as_bytes()));
                if !known_hashes.insert(content_hash.clone()) {
                    continue;
                }
                let publisher = publisher(&result.url);
                let title = if result.title.trim().is_empty() {
                    result.url.clone()
                } else {
                    result.title.trim().to_owned()
                };
                let content_path = format!("notes/{id}.md");

                collected.notes.push(NoteFile {
                    path: content_path.clone(),
                    content: content.clone(),
                });
                collected.sources.push(SourceRecord {
                    id,
                    subquestion_id: subquestion.id,
                    url: result.url,
                    canonical_url,
                    title,
                    publisher: publisher.clone(),
                    published_date: result.published_date,
                    retrieved_at: Utc::now(),
                    source_type: subquestion.source_class,
                    content_hash,
                    quality_rationale: format!(
                        "{} source selected for: {}",
                        subquestion.source_class.as_str(),
                        subquestion.question
                    ),
                    independence_group: publisher,
                    content_path,
                    excerpt: excerpt(&content),
                });
            }
        }

        Ok(collected)
    }

    fn search_request(&self, subquestion: &Subquestion, seed: &str) -> SearchRequest {
        let mut request = SearchRequest::new(seed)
            .with_depth(self.depth)
            .with_max_results(self.max_results);
        request.include_domains = self.constraints.include_domains.clone();
        request.exclude_domains = self.constraints.exclude_domains.clone();
        request.include_raw_content = Some(RawContentFormat::Markdown);
        request.include_published_date = true;

        if subquestion.source_class == SourceClass::News {
            request.topic = SearchTopic::News;
            request.time_range = Some(TimeRange::Month);
        }

        request
    }
}

fn publisher(raw_url: &str) -> String {
    Url::parse(raw_url)
        .ok()
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
        .map(|host| host.strip_prefix("www.").unwrap_or(&host).to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn excerpt(content: &str) -> String {
    let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = normalized.chars();
    let mut excerpt: String = chars.by_ref().take(300).collect();
    if chars.next().is_some() {
        excerpt.push('…');
    }
    excerpt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpts_are_single_line_and_bounded() {
        let short = excerpt("hello\nworld\tagain");
        assert_eq!(short, "hello world again");
        assert!(excerpt("x".repeat(500).as_str()).ends_with('…'));
    }

    #[test]
    fn publishers_drop_www() {
        assert_eq!(publisher("https://www.example.com/a"), "example.com");
        assert_eq!(publisher("not a url"), "unknown");
    }
}
