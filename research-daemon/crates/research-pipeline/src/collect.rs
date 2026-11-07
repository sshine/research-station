use std::collections::HashSet;

use chrono::Utc;
use research_core::{
    ExtractDepth, ExtractFormat, ExtractRequest, PlanConstraints, QueryId, QueryRecord,
    RawContentFormat, SearchDepth, SearchRequest, SearchTopic, SourceClass, SourceId, SourceRecord,
    Subquestion, TimeRange,
};
use research_core::{RedditSearchRequest, RedditThreadRequest};
use research_provider::{RedditProvider, SearchProvider};
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

            let call_limits = CollectionLimits {
                max_queries: limits.max_queries.saturating_sub(collected.queries.len()),
                max_sources: limits.max_sources.saturating_sub(collected.sources.len()),
                max_credits: if limits.max_credits == u64::MAX {
                    u64::MAX
                } else {
                    limits.max_credits.saturating_sub(credits_used)
                },
            };
            let request = self.search_request(subquestion, seed);
            let mut one = self
                .collect_query(subquestion, request, call_limits, known_urls, known_hashes)
                .await?;
            credits_used = credits_used.saturating_add(
                one.queries
                    .iter()
                    .filter_map(|query| query.credits)
                    .sum::<u64>(),
            );
            collected.queries.append(&mut one.queries);
            collected.sources.append(&mut one.sources);
            collected.notes.append(&mut one.notes);
        }

        Ok(collected)
    }

    /// Execute one search and accept its results. The agent loop calls this for
    /// each model-chosen query; `collect` calls it for each static seed.
    pub async fn collect_query(
        &self,
        subquestion: &Subquestion,
        request: SearchRequest,
        limits: CollectionLimits,
        known_urls: &mut HashSet<String>,
        known_hashes: &mut HashSet<String>,
    ) -> Result<CollectedSubquestion, PipelineError> {
        let mut collected = CollectedSubquestion::default();
        if limits.max_queries == 0 || limits.max_sources == 0 || limits.max_credits == 0 {
            return Ok(collected);
        }

        let text = request.query.clone();
        let response = self.provider.search(request).await?;
        collected.queries.push(QueryRecord {
            id: QueryId::new(),
            subquestion_id: subquestion.id,
            provider: response.provider.clone(),
            text,
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
            let content = result
                .raw_content
                .as_deref()
                .filter(|content| !content.trim().is_empty())
                .unwrap_or(&result.content);
            self.accept_source(
                subquestion,
                &result.url,
                &result.title,
                content,
                result.published_date,
                known_urls,
                known_hashes,
                &mut collected,
            );
        }

        Ok(collected)
    }

    /// Extract one or more URLs and accept the returned content. This is the
    /// page-reading action available to the agent loop.
    pub async fn collect_extract(
        &self,
        subquestion: &Subquestion,
        urls: Vec<String>,
        query: Option<String>,
        limits: CollectionLimits,
        known_urls: &mut HashSet<String>,
        known_hashes: &mut HashSet<String>,
    ) -> Result<CollectedSubquestion, PipelineError> {
        let mut collected = CollectedSubquestion::default();
        if urls.is_empty() {
            return Ok(collected);
        }
        if limits.max_queries == 0 || limits.max_sources == 0 || limits.max_credits == 0 {
            return Ok(collected);
        }

        let text = format!("extract: {}", urls.join(", "));
        let mut request = ExtractRequest::new(urls);
        request.query = query;
        request.format = ExtractFormat::Markdown;
        request.extract_depth = match self.depth {
            SearchDepth::Advanced => ExtractDepth::Advanced,
            _ => ExtractDepth::Basic,
        };

        let response = self.provider.extract(request).await?;
        let status = if response.data.failed_results.is_empty() {
            "ok"
        } else {
            "partial"
        };
        collected.queries.push(QueryRecord {
            id: QueryId::new(),
            subquestion_id: subquestion.id,
            provider: response.provider.clone(),
            text,
            at: Utc::now(),
            status: status.to_owned(),
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
            let title = title_from_url(&result.url);
            self.accept_source(
                subquestion,
                &result.url,
                &title,
                &result.raw_content,
                None,
                known_urls,
                known_hashes,
                &mut collected,
            );
        }

        Ok(collected)
    }

    #[allow(clippy::too_many_arguments)]
    fn accept_source(
        &self,
        subquestion: &Subquestion,
        url: &str,
        title: &str,
        content: &str,
        published_date: Option<String>,
        known_urls: &mut HashSet<String>,
        known_hashes: &mut HashSet<String>,
        collected: &mut CollectedSubquestion,
    ) {
        accept_source_record(
            subquestion,
            url,
            title,
            content,
            published_date,
            known_urls,
            known_hashes,
            collected,
        );
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

/// Execute one Reddit search and accept its normalized thread results.
///
/// The provider returns `SearchResponse` with the post body and, when
/// requested, the top comments already rendered into `content`. That keeps the
/// evidence ledger and the audit independent of the vendor response shape.
pub async fn collect_reddit<R: RedditProvider>(
    reddit: &R,
    subquestion: &Subquestion,
    request: RedditSearchRequest,
    limits: CollectionLimits,
    known_urls: &mut HashSet<String>,
    known_hashes: &mut HashSet<String>,
) -> Result<CollectedSubquestion, PipelineError> {
    let mut collected = CollectedSubquestion::default();
    if limits.max_queries == 0 || limits.max_sources == 0 || limits.max_credits == 0 {
        return Ok(collected);
    }

    let text = match &request.subreddit {
        Some(subreddit) => format!("reddit r/{subreddit}: {}", request.query),
        None => format!("reddit: {}", request.query),
    };
    let response = reddit.search(request).await?;
    collected.queries.push(QueryRecord {
        id: QueryId::new(),
        subquestion_id: subquestion.id,
        provider: response.provider.clone(),
        text,
        at: Utc::now(),
        status: "ok".to_owned(),
        result_count: response.data.results.len(),
        latency_ms: response.latency_ms,
        credits: response.credits,
        raw_hash: response.raw_hash.clone(),
        cache_hit: response.cache_hit,
    });

    // Reddit evidence is community evidence even when the pre-seed ran for a
    // subquestion the planner classified as reference or secondary.
    let mut community = subquestion.clone();
    community.source_class = SourceClass::Community;

    for result in response.data.results {
        if collected.sources.len() >= limits.max_sources {
            break;
        }
        let content = result
            .raw_content
            .as_deref()
            .filter(|content| !content.trim().is_empty())
            .unwrap_or(&result.content);
        accept_source_record(
            &community,
            &result.url,
            &result.title,
            content,
            result.published_date,
            known_urls,
            known_hashes,
            &mut collected,
        );
    }

    Ok(collected)
}

/// Fetch one Reddit thread and accept it as a source.
pub async fn collect_reddit_thread<R: RedditProvider>(
    reddit: &R,
    subquestion: &Subquestion,
    request: RedditThreadRequest,
    limits: CollectionLimits,
    known_urls: &mut HashSet<String>,
    known_hashes: &mut HashSet<String>,
) -> Result<CollectedSubquestion, PipelineError> {
    let mut collected = CollectedSubquestion::default();
    if limits.max_queries == 0 || limits.max_sources == 0 || limits.max_credits == 0 {
        return Ok(collected);
    }

    let text = format!("reddit thread: {}", request.id_or_url);
    let response = reddit.thread(request).await?;
    let mut community = subquestion.clone();
    community.source_class = SourceClass::Community;
    collected.queries.push(QueryRecord {
        id: QueryId::new(),
        subquestion_id: subquestion.id,
        provider: response.provider.clone(),
        text,
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
        let content = result
            .raw_content
            .as_deref()
            .filter(|content| !content.trim().is_empty())
            .unwrap_or(&result.content);
        accept_source_record(
            &community,
            &result.url,
            &result.title,
            content,
            result.published_date,
            known_urls,
            known_hashes,
            &mut collected,
        );
    }

    Ok(collected)
}

#[allow(clippy::too_many_arguments)]
fn accept_source_record(
    subquestion: &Subquestion,
    url: &str,
    title: &str,
    content: &str,
    published_date: Option<String>,
    known_urls: &mut HashSet<String>,
    known_hashes: &mut HashSet<String>,
    collected: &mut CollectedSubquestion,
) {
    let Some(canonical_url) = canonicalize_url(url) else {
        return;
    };
    if !known_urls.insert(canonical_url.clone()) {
        return;
    }

    let content = content.trim();
    if content.is_empty() {
        known_urls.remove(&canonical_url);
        return;
    }

    let id = SourceId::new();
    let content_hash = hex::encode(Sha256::digest(content.as_bytes()));
    if !known_hashes.insert(content_hash.clone()) {
        return;
    }
    let publisher = publisher(url);
    let title = if title.trim().is_empty() {
        url.to_owned()
    } else {
        title.trim().to_owned()
    };
    let content_path = format!("notes/{id}.md");

    collected.notes.push(NoteFile {
        path: content_path.clone(),
        content: content.to_owned(),
    });
    collected.sources.push(SourceRecord {
        id,
        subquestion_id: subquestion.id,
        url: url.to_owned(),
        canonical_url,
        title,
        publisher: publisher.clone(),
        published_date,
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
        excerpt: excerpt(content),
    });
}

fn publisher(raw_url: &str) -> String {
    let Ok(url) = Url::parse(raw_url) else {
        return "unknown".to_owned();
    };
    let Some(host) = url.host_str() else {
        return "unknown".to_owned();
    };
    let host = host
        .strip_prefix("www.")
        .unwrap_or(host)
        .to_ascii_lowercase();

    // Reddit threads are independent communities, not one publisher. Keeping
    // the subreddit in the independence group stops three threads from the same
    // community being counted as three independent sources.
    if host == "reddit.com" || host.ends_with(".reddit.com") {
        let segments: Vec<_> = url.path_segments().into_iter().flatten().collect();
        if let Some(position) = segments.iter().position(|segment| *segment == "r") {
            if let Some(subreddit) = segments.get(position + 1) {
                if !subreddit.is_empty() {
                    return format!("reddit.com/r/{subreddit}");
                }
            }
        }
    }

    host
}

fn title_from_url(raw_url: &str) -> String {
    Url::parse(raw_url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut segments| segments.next_back())
                .filter(|segment| !segment.is_empty())
                .map(|segment| segment.replace(['-', '_'], " "))
        })
        .map(|title| title.trim().to_owned())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| raw_url.to_owned())
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

    #[test]
    fn reddit_publishers_keep_the_subreddit() {
        assert_eq!(
            publisher("https://www.reddit.com/r/framework/comments/abc123/title/"),
            "reddit.com/r/framework"
        );
        assert_eq!(
            publisher("https://old.reddit.com/r/NixOS/comments/abc123/title/"),
            "reddit.com/r/NixOS"
        );
    }
}
