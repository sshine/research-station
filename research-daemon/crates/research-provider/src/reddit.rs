use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::StatusCode;
use reqwest::header::{ACCEPT, HeaderMap, RETRY_AFTER};
use research_core::{
    RedditConfig, RedditSearchMode, RedditSearchRequest, RedditSort, RedditThreadRequest,
    SearchResponse, SearchResult, Usage,
};
use research_store::{BlobStore, SqliteCache};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::time::Instant;

use crate::ProviderResponse;
use crate::budget::Budget;
use crate::error::ProviderError;
use crate::limiter::RateLimiter;

const PROVIDER: &str = "redditapis";
const FAKE_PROVIDER: &str = "reddit-fake";
const MAX_RESULTS: u8 = 100;
const MAX_COMMENTS: u8 = 100;

/// A Reddit search backend. Results are normalized to the same
/// `SearchResponse` shape the collector already understands, so the pipeline
/// can accept a thread as a source without vendor-specific types.
#[async_trait]
pub trait RedditProvider: Send + Sync {
    fn id(&self) -> &'static str;

    /// Hard cap on Reddit searches for one job, including pre-seeded searches.
    fn max_searches_per_job(&self) -> u32;

    async fn search(
        &self,
        request: RedditSearchRequest,
    ) -> Result<ProviderResponse<SearchResponse>, ProviderError>;

    async fn thread(
        &self,
        request: RedditThreadRequest,
    ) -> Result<ProviderResponse<SearchResponse>, ProviderError>;
}

/// Runtime selection between the real Reddit backend and the deterministic
/// fake used by tests and `--fake` runs.
#[derive(Clone)]
pub enum AnyReddit {
    Redditapis(RedditapisProvider),
    Fake(FakeRedditProvider),
}

impl AnyReddit {
    pub fn is_fake(&self) -> bool {
        matches!(self, Self::Fake(_))
    }
}

#[async_trait]
impl RedditProvider for AnyReddit {
    fn id(&self) -> &'static str {
        match self {
            Self::Redditapis(provider) => provider.id(),
            Self::Fake(provider) => provider.id(),
        }
    }

    fn max_searches_per_job(&self) -> u32 {
        match self {
            Self::Redditapis(provider) => provider.max_searches_per_job(),
            Self::Fake(provider) => provider.max_searches_per_job(),
        }
    }

    async fn search(
        &self,
        request: RedditSearchRequest,
    ) -> Result<ProviderResponse<SearchResponse>, ProviderError> {
        match self {
            Self::Redditapis(provider) => provider.search(request).await,
            Self::Fake(provider) => provider.search(request).await,
        }
    }

    async fn thread(
        &self,
        request: RedditThreadRequest,
    ) -> Result<ProviderResponse<SearchResponse>, ProviderError> {
        match self {
            Self::Redditapis(provider) => provider.thread(request).await,
            Self::Fake(provider) => provider.thread(request).await,
        }
    }
}

/// Redditapis.com search and thread reader.
#[derive(Clone)]
pub struct RedditapisProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    limiter: Arc<RateLimiter>,
    cache: Arc<SqliteCache>,
    blobs: Arc<BlobStore>,
    budget: Arc<Budget>,
    max_retries: u32,
    cache_ttl_seconds: u64,
    max_results: u8,
    include_comments: bool,
    comment_limit: u8,
    comment_thread_limit: u8,
    max_searches_per_job: u32,
}

impl RedditapisProvider {
    pub fn new(
        config: &RedditConfig,
        api_key: impl Into<String>,
        cache: Arc<SqliteCache>,
        blobs: Arc<BlobStore>,
        budget: Arc<Budget>,
    ) -> Result<Self, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds.max(1)))
            .user_agent(concat!(
                "research-station/",
                env!("CARGO_PKG_VERSION"),
                " (reddit-search)"
            ))
            .build()
            .map_err(|err| ProviderError::Transport(err.to_string()))?;

        Ok(Self {
            client,
            base_url: config.base_url.trim_end_matches('/').to_owned(),
            api_key: api_key.into(),
            limiter: Arc::new(RateLimiter::new(
                config.requests_per_minute,
                config.burst,
                config.max_concurrency,
            )),
            cache,
            blobs,
            budget,
            max_retries: config.max_retries,
            cache_ttl_seconds: config.cache_ttl_seconds,
            max_results: config.max_results.clamp(1, MAX_RESULTS),
            include_comments: config.include_comments,
            comment_limit: config.comment_limit.clamp(1, MAX_COMMENTS),
            comment_thread_limit: config.comment_thread_limit,
            max_searches_per_job: config.max_searches_per_job.max(1),
        })
    }

    async fn get_with_retry(&self, url: &str) -> Result<(Vec<u8>, Option<String>), ProviderError> {
        let mut attempt = 0u32;

        loop {
            let outcome = {
                let _permit = self.limiter.acquire().await;
                self.send_once(url).await
            };

            match outcome {
                Ok(bytes) => return Ok(bytes),
                Err(err) if err.is_retryable() && attempt < self.max_retries => {
                    let delay = err.retry_after().unwrap_or_else(|| backoff(attempt));
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
                Err(err) => return Err(err),
            }
        }
    }

    async fn send_once(&self, url: &str) -> Result<(Vec<u8>, Option<String>), ProviderError> {
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.api_key)
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(|err| ProviderError::Transport(err.to_string()))?;

        let status = response.status();
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);

        if status.is_success() {
            let bytes = response
                .bytes()
                .await
                .map(|bytes| bytes.to_vec())
                .map_err(|err| ProviderError::Transport(err.to_string()))?;
            return Ok((bytes, request_id));
        }

        let retry_after = parse_retry_after(response.headers());
        let message = response.text().await.unwrap_or_default();
        Err(classify(status, message, retry_after))
    }

    fn search_url(&self, request: &RedditSearchRequest) -> Result<String, ProviderError> {
        let path = match request.mode {
            RedditSearchMode::Posts => "/api/reddit/search",
            RedditSearchMode::Comments => "/api/reddit/search/comments",
        };
        let mut url = format!("{}{path}", self.base_url);
        let mut params: Vec<(String, String)> = vec![
            ("q".to_owned(), request.query.trim().to_owned()),
            ("sort".to_owned(), request.sort.as_str().to_owned()),
            (
                "limit".to_owned(),
                request
                    .max_results
                    .unwrap_or(self.max_results)
                    .clamp(1, MAX_RESULTS)
                    .to_string(),
            ),
        ];
        if let Some(subreddit) = &request.subreddit {
            params.push(("subreddit".to_owned(), normalize_subreddit(subreddit)?));
        }
        if let Some(time_range) = request.time_range {
            params.push(("t".to_owned(), time_range.as_str().to_owned()));
        }
        if request.mode == RedditSearchMode::Posts {
            if let Some(min_score) = request.min_score {
                params.push(("min_score".to_owned(), min_score.to_string()));
            }
        }
        append_query(&mut url, params);
        Ok(url)
    }

    fn thread_url(&self, request: &RedditThreadRequest) -> Result<String, ProviderError> {
        let id = extract_post_id(&request.id_or_url).ok_or_else(|| {
            ProviderError::InvalidRequest("not a Reddit post id or URL".to_owned())
        })?;
        let limit = request.limit.unwrap_or(100).clamp(1, MAX_COMMENTS);
        let mut url = format!("{}/api/reddit/post/{id}/comments", self.base_url);
        append_query(
            &mut url,
            vec![
                ("limit".to_owned(), limit.to_string()),
                ("sort".to_owned(), request.sort.as_str().to_owned()),
            ],
        );
        Ok(url)
    }

    async fn fetch_search(
        &self,
        request: &RedditSearchRequest,
    ) -> Result<(Vec<u8>, Option<String>), ProviderError> {
        let url = self.search_url(request)?;
        self.get_with_retry(&url).await
    }

    async fn fetch_thread_raw(
        &self,
        id_or_url: &str,
        limit: u8,
        sort: RedditSort,
    ) -> Result<(Vec<u8>, Option<String>), ProviderError> {
        let request = RedditThreadRequest {
            id_or_url: id_or_url.to_owned(),
            limit: Some(limit),
            sort,
        };
        let url = self.thread_url(&request)?;
        self.get_with_retry(&url).await
    }

    async fn normalize_search(
        &self,
        request: &RedditSearchRequest,
        bytes: &[u8],
        reads: &mut u64,
    ) -> Result<SearchResponse, ProviderError> {
        let listing: ListingResponse = serde_json::from_slice(bytes)?;
        let posts = listing.posts_for(request.mode);
        let max_results = request
            .max_results
            .unwrap_or(self.max_results)
            .clamp(1, MAX_RESULTS) as usize;
        let include_comments = request.include_comments.unwrap_or(self.include_comments);
        let comment_limit = request.comment_limit.unwrap_or(self.comment_limit);
        let comment_thread_limit = usize::from(self.comment_thread_limit);

        let mut results = Vec::with_capacity(posts.len().min(max_results));
        for (index, post) in posts.into_iter().take(max_results).enumerate() {
            let mut comments = Vec::new();
            let mut comment_note = None;

            if include_comments && index < comment_thread_limit {
                match self
                    .read_thread_comments(&post.id, comment_limit, request.sort, reads)
                    .await
                {
                    Ok((values, status, reason)) => {
                        comments = values;
                        comment_note = Some((status, reason));
                    }
                    Err(ProviderError::Budget(_)) => {
                        comment_note = Some((
                            Some("comments-skipped".to_owned()),
                            Some("budget exhausted".to_owned()),
                        ));
                    }
                    Err(err) => {
                        comment_note = Some((
                            Some("comments-unavailable".to_owned()),
                            Some(err.to_string()),
                        ));
                    }
                }
            }

            let mut content = format_thread(
                &post,
                &comments,
                comment_limit as usize,
                comment_note
                    .as_ref()
                    .and_then(|(status, _)| status.clone())
                    .or_else(|| listing.listing_status.clone()),
                comment_note
                    .as_ref()
                    .and_then(|(_, reason)| reason.clone())
                    .or_else(|| listing.exhausted_reason.clone()),
            );
            if request.mode == RedditSearchMode::Comments {
                content.push_str(
                    "\n<!-- found by comment-text search; this is the parent thread -->\n",
                );
            }

            results.push(SearchResult {
                title: post.title.clone(),
                url: post.thread_url(),
                content: content.clone(),
                score: post.upvotes.map(|score| score as f64),
                raw_content: Some(content),
                published_date: post.created.clone(),
                id: Some(post.id.clone()),
            });
        }

        Ok(SearchResponse {
            query: request.query.clone(),
            answer: None,
            results,
            response_time: None,
            request_id: None,
            usage: Some(Usage {
                credits: Some(*reads),
            }),
        })
    }

    async fn read_thread_comments(
        &self,
        id: &str,
        limit: u8,
        sort: RedditSort,
        reads: &mut u64,
    ) -> Result<(Vec<Value>, Option<String>, Option<String>), ProviderError> {
        let reservation = self.budget.try_reserve_query()?;
        let (bytes, _) = self.fetch_thread_raw(id, limit, sort).await?;
        reservation.commit();
        *reads = reads.saturating_add(1);

        let thread: ThreadResponse = serde_json::from_slice(&bytes)?;
        let _ = self.blobs.put(&bytes)?;
        Ok((
            thread.comments,
            thread.listing_status,
            thread.exhausted_reason,
        ))
    }
}

#[async_trait]
impl RedditProvider for RedditapisProvider {
    fn id(&self) -> &'static str {
        PROVIDER
    }

    fn max_searches_per_job(&self) -> u32 {
        self.max_searches_per_job
    }

    async fn search(
        &self,
        request: RedditSearchRequest,
    ) -> Result<ProviderResponse<SearchResponse>, ProviderError> {
        let started = Instant::now();
        validate_search(&request)?;
        let key = cache_key("search", &request)?;

        if let Some(cached) = self.cache.get(&key).await? {
            let data: SearchResponse = serde_json::from_str(&cached.response_json)?;
            let credits = data.usage.as_ref().and_then(|usage| usage.credits);
            return Ok(ProviderResponse {
                provider: PROVIDER.to_owned(),
                data,
                cache_hit: true,
                raw_hash: cached.raw_hash,
                credits,
                request_id: None,
                latency_ms: started.elapsed().as_millis() as u64,
            });
        }

        let reservation = self.budget.try_reserve_query()?;
        let (bytes, request_id) = self.fetch_search(&request).await?;
        reservation.commit();
        let mut reads = 1u64;
        let raw = self.blobs.put(&bytes)?;
        let data = self.normalize_search(&request, &bytes, &mut reads).await?;

        let request_json = serde_json::to_string(&request)?;
        let response_json = serde_json::to_string(&data)?;
        self.budget.record_credits(reads);
        self.cache
            .put(
                &key,
                PROVIDER,
                &request_json,
                &response_json,
                Some(&raw.hash),
                self.cache_ttl_seconds,
            )
            .await?;

        Ok(ProviderResponse {
            provider: PROVIDER.to_owned(),
            data,
            cache_hit: false,
            raw_hash: Some(raw.hash),
            credits: Some(reads),
            request_id,
            latency_ms: started.elapsed().as_millis() as u64,
        })
    }

    async fn thread(
        &self,
        request: RedditThreadRequest,
    ) -> Result<ProviderResponse<SearchResponse>, ProviderError> {
        let started = Instant::now();
        let id = extract_post_id(&request.id_or_url).ok_or_else(|| {
            ProviderError::InvalidRequest("not a Reddit post id or URL".to_owned())
        })?;
        let key = cache_key("thread", &(id.as_str(), request.limit, request.sort))?;

        if let Some(cached) = self.cache.get(&key).await? {
            let data: SearchResponse = serde_json::from_str(&cached.response_json)?;
            let credits = data.usage.as_ref().and_then(|usage| usage.credits);
            return Ok(ProviderResponse {
                provider: PROVIDER.to_owned(),
                data,
                cache_hit: true,
                raw_hash: cached.raw_hash,
                credits,
                request_id: None,
                latency_ms: started.elapsed().as_millis() as u64,
            });
        }

        let reservation = self.budget.try_reserve_query()?;
        let limit = request.limit.unwrap_or(100).clamp(1, MAX_COMMENTS);
        let (bytes, request_id) = self.fetch_thread_raw(&id, limit, request.sort).await?;
        reservation.commit();
        let reads = 1u64;
        let raw = self.blobs.put(&bytes)?;
        let thread: ThreadResponse = serde_json::from_slice(&bytes)?;
        let post = thread
            .post
            .ok_or_else(|| ProviderError::InvalidRequest(format!("post {id} not found")))?;
        let content = format_thread(
            &post,
            &thread.comments,
            usize::from(limit),
            thread.listing_status.clone(),
            thread.exhausted_reason.clone(),
        );
        let data = SearchResponse {
            query: format!("reddit thread {id}"),
            answer: None,
            results: vec![SearchResult {
                title: post.title.clone(),
                url: post.thread_url(),
                content: content.clone(),
                score: post.upvotes.map(|score| score as f64),
                raw_content: Some(content),
                published_date: post.created.clone(),
                id: Some(post.id.clone()),
            }],
            response_time: None,
            request_id: request_id.clone(),
            usage: Some(Usage {
                credits: Some(reads),
            }),
        };

        let request_json = serde_json::to_string(&request)?;
        let response_json = serde_json::to_string(&data)?;
        self.budget.record_credits(reads);
        self.cache
            .put(
                &key,
                PROVIDER,
                &request_json,
                &response_json,
                Some(&raw.hash),
                self.cache_ttl_seconds,
            )
            .await?;

        Ok(ProviderResponse {
            provider: PROVIDER.to_owned(),
            data,
            cache_hit: false,
            raw_hash: Some(raw.hash),
            credits: Some(reads),
            request_id,
            latency_ms: started.elapsed().as_millis() as u64,
        })
    }
}

/// Deterministic Reddit provider for tests and `--fake` runs.
#[derive(Debug, Clone, Default)]
pub struct FakeRedditProvider;

#[async_trait]
impl RedditProvider for FakeRedditProvider {
    fn id(&self) -> &'static str {
        FAKE_PROVIDER
    }

    fn max_searches_per_job(&self) -> u32 {
        4
    }

    async fn search(
        &self,
        request: RedditSearchRequest,
    ) -> Result<ProviderResponse<SearchResponse>, ProviderError> {
        let query = request.query.trim().to_owned();
        let slug = research_core::slugify(&query, 40);
        let suffix = hex::encode(Sha256::digest(query.as_bytes()));
        let suffix = &suffix[..8];
        let results = (1..=3)
            .map(|index| {
                let content = fake_thread_content(&query, index);
                SearchResult {
                    title: format!("{query} — fake Reddit thread {index}"),
                    url: format!(
                        "https://www.reddit.com/r/faketest/comments/{slug}-{suffix}-{index}/"
                    ),
                    content: content.clone(),
                    score: Some(120.0 - index as f64 * 10.0),
                    raw_content: Some(content),
                    published_date: Some("2026-10-01".to_owned()),
                    id: Some(format!("fake-reddit-{index}")),
                }
            })
            .collect();

        Ok(ProviderResponse {
            provider: FAKE_PROVIDER.to_owned(),
            data: SearchResponse {
                query,
                answer: None,
                results,
                response_time: Some(0.001),
                request_id: Some("fake-reddit".to_owned()),
                usage: Some(Usage { credits: Some(1) }),
            },
            cache_hit: false,
            raw_hash: None,
            credits: Some(1),
            request_id: Some("fake-reddit".to_owned()),
            latency_ms: 1,
        })
    }

    async fn thread(
        &self,
        request: RedditThreadRequest,
    ) -> Result<ProviderResponse<SearchResponse>, ProviderError> {
        let id = extract_post_id(&request.id_or_url).ok_or_else(|| {
            ProviderError::InvalidRequest("not a Reddit post id or URL".to_owned())
        })?;
        let content = fake_thread_content(&id, 1);
        Ok(ProviderResponse {
            provider: FAKE_PROVIDER.to_owned(),
            data: SearchResponse {
                query: format!("reddit thread {id}"),
                answer: None,
                results: vec![SearchResult {
                    title: format!("Fake Reddit thread {id}"),
                    url: format!("https://www.reddit.com/r/faketest/comments/{id}/"),
                    content: content.clone(),
                    score: Some(110.0),
                    raw_content: Some(content),
                    published_date: Some("2026-10-01".to_owned()),
                    id: Some(id),
                }],
                response_time: Some(0.001),
                request_id: Some("fake-reddit".to_owned()),
                usage: Some(Usage { credits: Some(1) }),
            },
            cache_hit: false,
            raw_hash: None,
            credits: Some(1),
            request_id: Some("fake-reddit".to_owned()),
            latency_ms: 1,
        })
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ListingResponse {
    #[serde(default)]
    posts: Vec<RedditPost>,
    #[serde(default)]
    comments: Vec<RedditPost>,
    #[serde(default)]
    listing_status: Option<String>,
    #[serde(default)]
    exhausted_reason: Option<String>,
}

impl ListingResponse {
    fn posts_for(&self, mode: RedditSearchMode) -> Vec<RedditPost> {
        match mode {
            RedditSearchMode::Posts => self.posts.clone(),
            RedditSearchMode::Comments => self.comments.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ThreadResponse {
    #[serde(default)]
    post: Option<RedditPost>,
    #[serde(default)]
    comments: Vec<Value>,
    #[serde(default)]
    listing_status: Option<String>,
    #[serde(default)]
    exhausted_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RedditPost {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    permalink: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default, alias = "selftext")]
    text: Option<String>,
    #[serde(default)]
    link_url: Option<String>,
    #[serde(default)]
    subreddit: String,
    #[serde(default)]
    upvotes: Option<i64>,
    #[serde(default, alias = "num_comments")]
    comments: Option<u64>,
    #[serde(default)]
    created: Option<String>,
}

impl RedditPost {
    fn thread_url(&self) -> String {
        if let Some(url) = &self.url {
            if !url.trim().is_empty() {
                return url.clone();
            }
        }
        if let Some(permalink) = &self.permalink {
            if permalink.starts_with("http") {
                return permalink.clone();
            }
            return format!("https://www.reddit.com{}", normalize_path(permalink));
        }
        format!("https://www.reddit.com/comments/{}/", self.id)
    }

    fn body(&self) -> &str {
        self.text.as_deref().unwrap_or("").trim()
    }
}

fn normalize_path(path: &str) -> String {
    if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    }
}

fn format_thread(
    post: &RedditPost,
    comments: &[Value],
    comment_limit: usize,
    listing_status: Option<String>,
    exhausted_reason: Option<String>,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", post.title.trim()));
    out.push_str(&format!("- Subreddit: r/{}\n", post.subreddit));
    if let Some(author) = &post.author {
        out.push_str(&format!("- Author: u/{author}\n"));
    }
    if let Some(score) = post.upvotes {
        out.push_str(&format!("- Score: {score}\n"));
    }
    if let Some(count) = post.comments {
        out.push_str(&format!("- Comments: {count}\n"));
    }
    if let Some(created) = &post.created {
        out.push_str(&format!("- Created: {created}\n"));
    }
    out.push_str(&format!("- URL: {}\n", post.thread_url()));
    if let Some(link) = &post.link_url {
        if !link.trim().is_empty() && Some(link.as_str()) != post.url.as_deref() {
            out.push_str(&format!("- Link: {link}\n"));
        }
    }
    out.push('\n');

    let body = post.body();
    if body.is_empty() {
        out.push_str("_(link post or empty selftext)_\n");
    } else {
        out.push_str(body);
        out.push('\n');
    }

    let mut views = Vec::new();
    let mut truncated = false;
    walk_comments(comments, 0, &mut views, comment_limit, &mut truncated);
    if !views.is_empty() {
        out.push_str("\n## Comments\n");
        for view in &views {
            out.push_str(&format!(
                "\n### u/{} · {} points{}\n{}\n",
                view.author.as_deref().unwrap_or("unknown"),
                view.score.unwrap_or(0),
                view.created
                    .as_deref()
                    .map(|created| format!(" · {created}"))
                    .unwrap_or_default(),
                if view.depth == 0 {
                    view.body.clone()
                } else {
                    format!("{}> {}", "  ".repeat(usize::from(view.depth)), view.body)
                }
            ));
        }
    }
    if truncated {
        out.push_str("\n<!-- more comments omitted -->\n");
    }
    if let Some(status) = listing_status {
        out.push_str(&format!("\n<!-- listing_status: {status} -->\n"));
    }
    if let Some(reason) = exhausted_reason {
        out.push_str(&format!("<!-- exhausted_reason: {reason} -->\n"));
    }
    out
}

#[derive(Debug, Clone)]
struct CommentView {
    author: Option<String>,
    score: Option<i64>,
    created: Option<String>,
    body: String,
    depth: u8,
}

fn walk_comments(
    nodes: &[Value],
    depth: u8,
    out: &mut Vec<CommentView>,
    limit: usize,
    truncated: &mut bool,
) {
    for node in nodes {
        if out.len() >= limit {
            *truncated = true;
            break;
        }
        let kind = node.get("kind").and_then(Value::as_str).unwrap_or_default();
        if kind == "more" {
            *truncated = true;
            continue;
        }
        if kind != "t1" {
            continue;
        }
        let Some(data) = node.get("data") else {
            continue;
        };
        let body = data
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if !body.is_empty() {
            out.push(CommentView {
                author: data
                    .get("author")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                score: data
                    .get("score")
                    .and_then(Value::as_i64)
                    .or_else(|| data.get("ups").and_then(Value::as_i64)),
                created: data
                    .get("created")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
                    .or_else(|| {
                        data.get("created_utc")
                            .and_then(Value::as_i64)
                            .map(|epoch| epoch.to_string())
                    }),
                body: body.to_owned(),
                depth,
            });
        }
        if depth < 2 {
            if let Some(children) = data
                .get("replies")
                .and_then(Value::as_object)
                .and_then(|replies| replies.get("data"))
                .and_then(|data| data.get("children"))
                .and_then(Value::as_array)
            {
                walk_comments(children, depth + 1, out, limit, truncated);
            }
        }
    }
}

fn fake_thread_content(query: &str, index: usize) -> String {
    format!(
        "# {query} — fake Reddit thread {index}\n\n\
         - Subreddit: r/faketest\n\
         - Author: u/fake_researcher\n\
         - Score: 120\n\
         - Comments: 37\n\
         - Created: 2026-10-01\n\
         - URL: https://www.reddit.com/r/faketest/comments/fake-{index}/\n\n\
         I have used {query} for six months and the main trade-off is predictable cost versus \
         operational complexity. The documentation is clear, but the failure modes are not always \
         obvious. A second practitioner reports that {query} works well for small teams and becomes \
         harder to operate at scale. A third comment says the opposite: the tool was reliable for \
         them, but support was slow.\n\n\
         ## Comments\n\n\
         ### u/practitioner_one · 45 points · 2026-10-02\n\
         In practice {query} is solid, but you need to budget time for setup and monitoring.\n\n\
         ### u/practitioner_two · 22 points · 2026-10-02\n\
         My experience with {query} was mixed. It solved the main problem, but the operational cost \
         was higher than expected.\n"
    )
}

fn validate_search(request: &RedditSearchRequest) -> Result<(), ProviderError> {
    if request.query.trim().is_empty() {
        return Err(ProviderError::InvalidRequest(
            "query must not be empty".to_owned(),
        ));
    }
    if let Some(max_results) = request.max_results {
        if !(1..=MAX_RESULTS).contains(&max_results) {
            return Err(ProviderError::InvalidRequest(format!(
                "max_results must be between 1 and {MAX_RESULTS}, got {max_results}"
            )));
        }
    }
    if let Some(comment_limit) = request.comment_limit {
        if !(1..=MAX_COMMENTS).contains(&comment_limit) {
            return Err(ProviderError::InvalidRequest(format!(
                "comment_limit must be between 1 and {MAX_COMMENTS}, got {comment_limit}"
            )));
        }
    }
    if let Some(subreddit) = &request.subreddit {
        normalize_subreddit(subreddit)?;
    }
    Ok(())
}

fn normalize_subreddit(subreddit: &str) -> Result<String, ProviderError> {
    let name = subreddit
        .trim()
        .strip_prefix("r/")
        .unwrap_or(subreddit.trim())
        .trim();
    if name.is_empty()
        || name.len() > 100
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(ProviderError::InvalidRequest(format!(
            "invalid subreddit name: {subreddit}"
        )));
    }
    Ok(name.to_owned())
}

fn extract_post_id(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate = if let Some(position) = trimmed.find("/comments/") {
        let rest = &trimmed[position + "/comments/".len()..];
        rest.split(['/', '?', '#']).next().unwrap_or("")
    } else {
        trimmed.strip_prefix("t3_").unwrap_or(trimmed)
    };
    if (1..=16).contains(&candidate.len())
        && candidate
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        Some(candidate.to_ascii_lowercase())
    } else {
        None
    }
}

fn append_query(url: &mut String, params: Vec<(String, String)>) {
    let pairs: Vec<String> = params
        .into_iter()
        .map(|(key, value)| format!("{}={}", urlencode(&key), urlencode(&value)))
        .collect();
    if !pairs.is_empty() {
        url.push('?');
        url.push_str(&pairs.join("&"));
    }
}

fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn cache_key<T: Serialize>(operation: &str, request: &T) -> Result<String, ProviderError> {
    #[derive(Serialize)]
    struct Key<'a, T> {
        provider: &'static str,
        operation: &'a str,
        request: &'a T,
    }

    let bytes = serde_json::to_vec(&Key {
        provider: PROVIDER,
        operation,
        request,
    })?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn classify(status: StatusCode, message: String, retry_after: Option<Duration>) -> ProviderError {
    match status.as_u16() {
        401 | 403 => ProviderError::Auth(message),
        400 | 404 | 422 => ProviderError::InvalidRequest(message),
        402 => ProviderError::QuotaExceeded(message),
        408 => ProviderError::Transient(format!("408: {message}")),
        429 => ProviderError::RateLimited {
            message,
            retry_after,
        },
        502..=504 => ProviderError::Transient(format!("{status}: {message}")),
        500 => ProviderError::Transient(format!("500: {message}")),
        code => ProviderError::Api {
            status: code,
            message,
        },
    }
}

fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

fn backoff(attempt: u32) -> Duration {
    let base = 250u64.saturating_mul(1u64 << attempt.min(6));
    let jitter = fastrand::u64(0..=250);
    Duration::from_millis((base + jitter).min(30_000))
}

#[cfg(test)]
mod tests {
    use super::*;
    use research_core::TimeRange;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    struct Harness {
        provider: RedditapisProvider,
        _dir: TempDir,
        budget: Arc<Budget>,
        server: MockServer,
    }

    async fn harness(budget: Budget) -> Harness {
        let server = MockServer::start().await;
        let dir = tempfile::tempdir().unwrap();
        let cache = Arc::new(
            SqliteCache::open(dir.path().join("cache.sqlite"))
                .await
                .unwrap(),
        );
        let blobs = Arc::new(BlobStore::new(dir.path().join("blobs")));
        let config = RedditConfig {
            base_url: server.uri(),
            requests_per_minute: 6_000,
            burst: 100,
            max_concurrency: 4,
            max_retries: 1,
            timeout_seconds: 5,
            cache_ttl_seconds: 3_600,
            include_comments: false,
            ..RedditConfig::default()
        };
        let budget = Arc::new(budget);
        let provider =
            RedditapisProvider::new(&config, "test-reddit-key", cache, blobs, budget.clone())
                .unwrap();
        Harness {
            provider,
            _dir: dir,
            budget,
            server,
        }
    }

    fn search_response() -> Value {
        serde_json::json!({
            "posts": [{
                "id": "abc123",
                "title": "Framework laptop problems",
                "author": "someone",
                "permalink": "/r/framework/comments/abc123/framework_laptop_problems/",
                "url": "https://www.reddit.com/r/framework/comments/abc123/framework_laptop_problems/",
                "text": "My Framework laptop has a fan noise problem after six months.",
                "subreddit": "framework",
                "upvotes": 42,
                "comments": 12,
                "created": "2026-01-01T00:00:00Z"
            }],
            "after": null,
            "listing_status": "complete"
        })
    }

    fn thread_response() -> Value {
        serde_json::json!({
            "post": {
                "id": "abc123",
                "title": "Framework laptop problems",
                "author": "someone",
                "url": "https://www.reddit.com/r/framework/comments/abc123/framework_laptop_problems/",
                "text": "My Framework laptop has a fan noise problem after six months.",
                "subreddit": "framework",
                "upvotes": 42,
                "comments": 12,
                "created": "2026-01-01T00:00:00Z"
            },
            "comments": [{
                "kind": "t1",
                "data": {
                    "id": "c1",
                    "author": "commenter",
                    "score": 9,
                    "created_utc": 1767225600,
                    "body": "The fan noise is a known issue; support replaced the mainboard.",
                    "replies": {
                        "kind": "Listing",
                        "data": {
                            "children": [{
                                "kind": "t1",
                                "data": {
                                    "id": "c2",
                                    "author": "repairperson",
                                    "score": 3,
                                    "body": "Replacing the thermal paste helped mine.",
                                    "replies": ""
                                }
                            }]
                        }
                    }
                }
            }],
            "listing_status": "unknown",
            "exhausted_reason": "comment_tree"
        })
    }

    struct FirstRateLimited {
        calls: AtomicUsize,
    }

    impl wiremock::Respond for FirstRateLimited {
        fn respond(&self, _request: &wiremock::Request) -> ResponseTemplate {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                ResponseTemplate::new(429)
                    .insert_header("retry-after", "0")
                    .set_body_json(serde_json::json!({"error": "rate_limited"}))
            } else {
                ResponseTemplate::new(200).set_body_json(search_response())
            }
        }
    }

    #[tokio::test]
    async fn search_maps_request_and_caches_response() {
        let h = harness(Budget::new(10, 100)).await;
        Mock::given(method("GET"))
            .and(path("/api/reddit/search"))
            .and(header("authorization", "Bearer test-reddit-key"))
            .and(query_param("q", "framework laptop"))
            .and(query_param("sort", "relevance"))
            .and(query_param("t", "year"))
            .respond_with(ResponseTemplate::new(200).set_body_json(search_response()))
            .expect(1)
            .mount(&h.server)
            .await;

        let request = RedditSearchRequest {
            time_range: Some(TimeRange::Year),
            ..RedditSearchRequest::new("framework laptop")
        };
        let first = h.provider.search(request.clone()).await.unwrap();
        assert_eq!(first.data.results[0].title, "Framework laptop problems");
        assert!(first.data.results[0].content.contains("fan noise"));
        assert_eq!(first.credits, Some(1));
        assert!(!first.cache_hit);
        assert_eq!(h.budget.queries_used(), 1);

        let second = h.provider.search(request).await.unwrap();
        assert!(second.cache_hit);
        assert_eq!(
            second.data.results[0].url,
            "https://www.reddit.com/r/framework/comments/abc123/framework_laptop_problems/"
        );
        assert_eq!(h.server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn search_fetches_comments_for_top_threads() {
        let h = harness(Budget::new(10, 100)).await;
        Mock::given(method("GET"))
            .and(path("/api/reddit/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(search_response()))
            .mount(&h.server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/reddit/post/abc123/comments"))
            .and(query_param("limit", "20"))
            .respond_with(ResponseTemplate::new(200).set_body_json(thread_response()))
            .expect(1)
            .mount(&h.server)
            .await;

        let request = RedditSearchRequest::new("framework laptop").with_comments(20);
        let response = h.provider.search(request).await.unwrap();
        let content = &response.data.results[0].content;
        assert!(content.contains("known issue"));
        assert!(content.contains("thermal paste"));
        assert_eq!(response.credits, Some(2));
        assert_eq!(h.budget.queries_used(), 2);
    }

    #[tokio::test]
    async fn comment_search_reads_the_post_array() {
        let h = harness(Budget::new(10, 100)).await;
        Mock::given(method("GET"))
            .and(path("/api/reddit/search/comments"))
            .and(query_param("q", "fan noise"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "comments": [{
                    "id": "abc123",
                    "title": "Framework laptop problems",
                    "author": "someone",
                    "subreddit": "framework",
                    "upvotes": 42,
                    "num_comments": 12,
                    "selftext": "My Framework laptop has a fan noise problem.",
                    "url": "https://www.reddit.com/r/framework/comments/abc123/framework_laptop_problems/",
                    "created": "2026-01-01T00:00:00Z"
                }]
            })))
            .mount(&h.server)
            .await;

        let request = RedditSearchRequest {
            mode: RedditSearchMode::Comments,
            ..RedditSearchRequest::new("fan noise")
        };
        let response = h.provider.search(request).await.unwrap();
        assert_eq!(response.data.results.len(), 1);
        assert!(response.data.results[0].content.contains("fan noise"));
    }

    #[tokio::test]
    async fn thread_fetches_full_tree_and_flattens_replies() {
        let h = harness(Budget::new(10, 100)).await;
        Mock::given(method("GET"))
            .and(path("/api/reddit/post/abc123/comments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(thread_response()))
            .expect(1)
            .mount(&h.server)
            .await;

        let response = h
            .provider
            .thread(RedditThreadRequest::new(
                "https://www.reddit.com/r/framework/comments/abc123/framework_laptop_problems/",
            ))
            .await
            .unwrap();
        let content = &response.data.results[0].content;
        assert!(content.contains("known issue"));
        assert!(content.contains("thermal paste"));
        assert!(content.contains("listing_status: unknown"));
    }

    #[tokio::test]
    async fn invalid_requests_do_not_hit_the_provider() {
        let h = harness(Budget::new(10, 100)).await;
        let err = h
            .provider
            .search(RedditSearchRequest::new(" "))
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::InvalidRequest(_)));
        assert_eq!(h.server.received_requests().await.unwrap().len(), 0);

        let err = h
            .provider
            .thread(RedditThreadRequest::new("not a post"))
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::InvalidRequest(_)));
    }

    #[tokio::test]
    async fn query_budget_stops_before_sending() {
        let h = harness(Budget::new(1, 100)).await;
        Mock::given(method("GET"))
            .and(path("/api/reddit/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(search_response()))
            .expect(1)
            .mount(&h.server)
            .await;

        h.provider
            .search(RedditSearchRequest::new("first"))
            .await
            .unwrap();

        let err = h
            .provider
            .search(RedditSearchRequest::new("second"))
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::Budget(_)));
    }

    #[tokio::test]
    async fn retries_rate_limited_requests() {
        let h = harness(Budget::new(10, 100)).await;
        Mock::given(method("GET"))
            .and(path("/api/reddit/search"))
            .respond_with(FirstRateLimited {
                calls: AtomicUsize::new(0),
            })
            .expect(2)
            .mount(&h.server)
            .await;

        let response = h
            .provider
            .search(RedditSearchRequest::new("retry me"))
            .await
            .unwrap();
        assert_eq!(response.data.results[0].title, "Framework laptop problems");
        assert_eq!(h.server.received_requests().await.unwrap().len(), 2);
    }

    #[test]
    fn extracts_post_ids_from_ids_and_urls() {
        assert_eq!(extract_post_id("abc123").as_deref(), Some("abc123"));
        assert_eq!(extract_post_id("t3_ABC123").as_deref(), Some("abc123"));
        assert_eq!(
            extract_post_id("https://www.reddit.com/r/framework/comments/abc123/title/").as_deref(),
            Some("abc123")
        );
        assert!(extract_post_id("not a post").is_none());
    }

    #[test]
    fn fake_provider_is_deterministic_and_quotable() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let response = runtime
            .block_on(FakeRedditProvider.search(RedditSearchRequest::new("Notion")))
            .unwrap();
        assert_eq!(response.data.results.len(), 3);
        assert!(
            response.data.results[0]
                .content
                .contains("predictable cost versus operational complexity")
        );
    }
}
