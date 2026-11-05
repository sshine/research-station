use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::StatusCode;
use reqwest::header::{CONTENT_TYPE, HeaderMap, RETRY_AFTER};
use research_core::{
    ExtractDepth, ExtractRequest, ExtractResponse, SearchRequest, SearchResponse, TavilyConfig,
};
use research_store::{BlobStore, SqliteCache};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::time::Instant;

use crate::budget::Budget;
use crate::error::ProviderError;
use crate::limiter::RateLimiter;
use crate::{ProviderResponse, SearchProvider};

const PROVIDER: &str = "tavily";

/// Tavily search and extract provider.
#[derive(Clone)]
pub struct TavilyProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    limiter: Arc<RateLimiter>,
    cache: Arc<SqliteCache>,
    blobs: Arc<BlobStore>,
    budget: Arc<Budget>,
    max_retries: u32,
    cache_ttl_seconds: u64,
}

impl TavilyProvider {
    pub fn new(
        config: &TavilyConfig,
        api_key: impl Into<String>,
        cache: Arc<SqliteCache>,
        blobs: Arc<BlobStore>,
        budget: Arc<Budget>,
    ) -> Result<Self, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds.max(1)))
            .user_agent(concat!("research-station/", env!("CARGO_PKG_VERSION")))
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
        })
    }

    async fn post_with_retry(&self, path: &str, body: &[u8]) -> Result<Vec<u8>, ProviderError> {
        let mut attempt = 0u32;

        loop {
            let outcome = {
                let _permit = self.limiter.acquire().await;
                self.send_once(path, body).await
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

    async fn send_once(&self, path: &str, body: &[u8]) -> Result<Vec<u8>, ProviderError> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .header(CONTENT_TYPE, "application/json")
            .body(body.to_vec())
            .send()
            .await
            .map_err(|err| ProviderError::Transport(err.to_string()))?;

        let status = response.status();
        if status.is_success() {
            return response
                .bytes()
                .await
                .map(|bytes| bytes.to_vec())
                .map_err(|err| ProviderError::Transport(err.to_string()));
        }

        let retry_after = parse_retry_after(response.headers());
        let message = response.text().await.unwrap_or_default();
        Err(classify(status, message, retry_after))
    }
}

#[async_trait]
impl SearchProvider for TavilyProvider {
    fn id(&self) -> &'static str {
        PROVIDER
    }

    async fn search(
        &self,
        request: SearchRequest,
    ) -> Result<ProviderResponse<SearchResponse>, ProviderError> {
        let started = Instant::now();
        let key = cache_key("search", &request)?;

        if let Some(cached) = self.cache.get(&key).await? {
            let data: SearchResponse = serde_json::from_str(&cached.response_json)?;
            let credits = data.usage.as_ref().and_then(|usage| usage.credits);
            let request_id = data.request_id.clone();
            return Ok(ProviderResponse {
                provider: PROVIDER.to_owned(),
                data,
                cache_hit: true,
                raw_hash: cached.raw_hash,
                credits,
                request_id,
                latency_ms: started.elapsed().as_millis() as u64,
            });
        }

        let reservation = self.budget.try_reserve_query()?;
        let body = search_body(&request)?;
        let body_bytes = serde_json::to_vec(&body)?;
        let bytes = self.post_with_retry("/search", &body_bytes).await?;

        // The query was billed once the provider answered, even if decoding or
        // caching fails afterwards. Commit before post-processing.
        reservation.commit();

        let estimated_credits = request.search_depth.estimated_credits();
        let data: SearchResponse = match serde_json::from_slice(&bytes) {
            Ok(data) => data,
            Err(err) => {
                self.budget.record_credits(estimated_credits);
                return Err(err.into());
            }
        };

        let credits = data
            .usage
            .as_ref()
            .and_then(|usage| usage.credits)
            .unwrap_or(estimated_credits);
        let request_id = data.request_id.clone();
        let raw = self.blobs.put(&bytes)?;
        let request_json = serde_json::to_string(&request)?;
        let response_json = String::from_utf8_lossy(&bytes).into_owned();

        self.budget.record_credits(credits);
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
            credits: Some(credits),
            request_id,
            latency_ms: started.elapsed().as_millis() as u64,
        })
    }

    async fn extract(
        &self,
        request: ExtractRequest,
    ) -> Result<ProviderResponse<ExtractResponse>, ProviderError> {
        let started = Instant::now();
        let key = cache_key("extract", &request)?;

        if let Some(cached) = self.cache.get(&key).await? {
            let data: ExtractResponse = serde_json::from_str(&cached.response_json)?;
            let credits = data.usage.as_ref().and_then(|usage| usage.credits);
            let request_id = data.request_id.clone();
            return Ok(ProviderResponse {
                provider: PROVIDER.to_owned(),
                data,
                cache_hit: true,
                raw_hash: cached.raw_hash,
                credits,
                request_id,
                latency_ms: started.elapsed().as_millis() as u64,
            });
        }

        let reservation = self.budget.try_reserve_query()?;
        let body = extract_body(&request)?;
        let body_bytes = serde_json::to_vec(&body)?;
        let bytes = self.post_with_retry("/extract", &body_bytes).await?;

        reservation.commit();

        let estimated_credits =
            estimated_extract_credits(request.extract_depth, request.urls.len());
        let data: ExtractResponse = match serde_json::from_slice(&bytes) {
            Ok(data) => data,
            Err(err) => {
                self.budget.record_credits(estimated_credits);
                return Err(err.into());
            }
        };

        let credits = data
            .usage
            .as_ref()
            .and_then(|usage| usage.credits)
            .unwrap_or(estimated_credits);
        let request_id = data.request_id.clone();
        let raw = self.blobs.put(&bytes)?;
        let request_json = serde_json::to_string(&request)?;
        let response_json = String::from_utf8_lossy(&bytes).into_owned();

        self.budget.record_credits(credits);
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
            credits: Some(credits),
            request_id,
            latency_ms: started.elapsed().as_millis() as u64,
        })
    }
}

fn search_body(request: &SearchRequest) -> Result<Value, ProviderError> {
    if request.query.trim().is_empty() {
        return Err(ProviderError::InvalidRequest(
            "query must not be empty".to_owned(),
        ));
    }
    if let Some(max_results) = request.max_results {
        if !(1..=20).contains(&max_results) {
            return Err(ProviderError::InvalidRequest(format!(
                "max_results must be between 1 and 20, got {max_results}"
            )));
        }
    }
    if request.include_domains.len() > 300 {
        return Err(ProviderError::InvalidRequest(
            "include_domains accepts at most 300 domains".to_owned(),
        ));
    }
    if request.exclude_domains.len() > 150 {
        return Err(ProviderError::InvalidRequest(
            "exclude_domains accepts at most 150 domains".to_owned(),
        ));
    }

    let mut body = json!({
        "query": request.query.clone(),
        "search_depth": request.search_depth.as_str(),
        "topic": request.topic.as_str(),
        "include_usage": true,
    });

    if let Some(max_results) = request.max_results {
        body["max_results"] = json!(max_results);
    }
    if !request.include_domains.is_empty() {
        body["include_domains"] = json!(request.include_domains.clone());
    }
    if !request.exclude_domains.is_empty() {
        body["exclude_domains"] = json!(request.exclude_domains.clone());
    }
    if let Some(time_range) = request.time_range {
        body["time_range"] = json!(time_range.as_str());
    }
    if let Some(start_date) = request.start_date {
        body["start_date"] = json!(start_date.to_string());
    }
    if let Some(end_date) = request.end_date {
        body["end_date"] = json!(end_date.to_string());
    }
    if let Some(raw_content) = request.include_raw_content {
        body["include_raw_content"] = json!(raw_content.as_str());
    }
    if request.include_published_date {
        body["include_published_date"] = json!(true);
    }
    if request.filter_by_published_date {
        body["filter_by_published_date"] = json!(true);
    }
    if let Some(country) = &request.country {
        body["country"] = json!(country);
    }

    Ok(body)
}

fn extract_body(request: &ExtractRequest) -> Result<Value, ProviderError> {
    if request.urls.is_empty() {
        return Err(ProviderError::InvalidRequest(
            "at least one URL is required".to_owned(),
        ));
    }
    if request.urls.len() > 20 {
        return Err(ProviderError::InvalidRequest(
            "extract accepts at most 20 URLs".to_owned(),
        ));
    }
    if let Some(timeout) = request.timeout_seconds {
        if !(1.0..=60.0).contains(&timeout) {
            return Err(ProviderError::InvalidRequest(
                "extract timeout must be between 1 and 60 seconds".to_owned(),
            ));
        }
    }

    let mut body = json!({
        "urls": request.urls.clone(),
        "extract_depth": request.extract_depth.as_str(),
        "format": request.format.as_str(),
        "include_images": request.include_images,
        "include_usage": true,
    });

    if let Some(query) = &request.query {
        body["query"] = json!(query);
    }
    if let Some(timeout) = request.timeout_seconds {
        body["timeout"] = json!(timeout);
    }

    Ok(body)
}

fn estimated_extract_credits(depth: ExtractDepth, urls: usize) -> u64 {
    let batches = urls.div_ceil(5) as u64;
    depth.credits_per_five_urls() * batches
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
        400 | 422 => ProviderError::InvalidRequest(message),
        429 => ProviderError::RateLimited {
            message,
            retry_after,
        },
        432 | 433 => ProviderError::QuotaExceeded(message),
        434 => ProviderError::CapacityExceeded(message),
        500..=599 => ProviderError::Transient(format!("{status}: {message}")),
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
    use research_core::{ExtractFormat, SearchDepth};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    struct FirstRateLimited {
        calls: AtomicUsize,
    }

    impl wiremock::Respond for FirstRateLimited {
        fn respond(&self, _request: &wiremock::Request) -> ResponseTemplate {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                ResponseTemplate::new(429)
                    .insert_header("retry-after", "0")
                    .set_body_json(json!({"detail": {"error": "slow down"}}))
            } else {
                ResponseTemplate::new(200).set_body_json(search_response())
            }
        }
    }

    struct Harness {
        provider: TavilyProvider,
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
        let config = TavilyConfig {
            base_url: server.uri(),
            requests_per_minute: 6_000,
            burst: 100,
            max_concurrency: 4,
            max_retries: 1,
            timeout_seconds: 5,
            cache_ttl_seconds: 3_600,
            ..TavilyConfig::default()
        };
        let budget = Arc::new(budget);
        let provider =
            TavilyProvider::new(&config, "test-key", cache, blobs, budget.clone()).unwrap();

        Harness {
            provider,
            _dir: dir,
            budget,
            server,
        }
    }

    fn search_response() -> Value {
        json!({
            "query": "test",
            "answer": null,
            "results": [{
                "title": "Example",
                "url": "https://example.com",
                "content": "Example content",
                "score": 0.9,
                "raw_content": "# Example",
                "published_date": "Mon, 02 Nov 2026 00:00:00 GMT",
                "id": "r1"
            }],
            "response_time": 0.1,
            "request_id": "req-1",
            "usage": { "credits": 1 }
        })
    }

    #[tokio::test]
    async fn search_maps_request_and_caches_response() {
        let h = harness(Budget::new(10, 100)).await;
        Mock::given(method("POST"))
            .and(path("/search"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(search_response()))
            .expect(1)
            .mount(&h.server)
            .await;

        let request = SearchRequest::new("test")
            .with_depth(SearchDepth::Advanced)
            .with_max_results(5);
        let first = h.provider.search(request.clone()).await.unwrap();

        assert_eq!(first.data.results[0].title, "Example");
        assert_eq!(first.credits, Some(1));
        assert!(!first.cache_hit);
        assert!(first.raw_hash.is_some());
        assert_eq!(h.budget.queries_used(), 1);

        let sent = h.server.received_requests().await.unwrap();
        assert_eq!(sent.len(), 1);
        let body: Value = serde_json::from_slice(&sent[0].body).unwrap();
        assert_eq!(body["search_depth"], "advanced");
        assert_eq!(body["max_results"], 5);
        assert_eq!(body["include_usage"], true);

        let second = h.provider.search(request).await.unwrap();
        assert!(second.cache_hit);
        assert_eq!(second.data.results[0].url, "https://example.com");
        assert_eq!(h.server.received_requests().await.unwrap().len(), 1);
        assert_eq!(h.budget.queries_used(), 1);
    }

    #[tokio::test]
    async fn extract_maps_request_and_reports_failures() {
        let h = harness(Budget::new(10, 100)).await;
        Mock::given(method("POST"))
            .and(path("/extract"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "results": [{
                    "url": "https://example.com/a",
                    "raw_content": "# A"
                }],
                "failed_results": [{
                    "url": "https://example.com/b",
                    "error": "timeout"
                }],
                "response_time": 0.2,
                "request_id": "req-2",
                "usage": { "credits": 1 }
            })))
            .expect(1)
            .mount(&h.server)
            .await;

        let mut request = ExtractRequest::new(["https://example.com/a", "https://example.com/b"]);
        request.extract_depth = ExtractDepth::Advanced;
        request.format = ExtractFormat::Text;
        request.query = Some("focus".to_owned());
        request.timeout_seconds = Some(10.0);

        let result = h.provider.extract(request).await.unwrap();
        assert_eq!(result.data.results[0].raw_content, "# A");
        assert_eq!(result.data.failed_results[0].error, "timeout");
        assert_eq!(result.credits, Some(1));

        let sent = h.server.received_requests().await.unwrap();
        let body: Value = serde_json::from_slice(&sent[0].body).unwrap();
        assert_eq!(body["extract_depth"], "advanced");
        assert_eq!(body["format"], "text");
        assert_eq!(body["timeout"], 10.0);
    }

    #[tokio::test]
    async fn invalid_requests_do_not_hit_the_provider() {
        let h = harness(Budget::new(10, 100)).await;
        let err = h
            .provider
            .search(SearchRequest::new("test").with_max_results(21))
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::InvalidRequest(_)));
        assert_eq!(h.server.received_requests().await.unwrap().len(), 0);

        let err = h
            .provider
            .extract(ExtractRequest::new(std::iter::empty::<&str>()))
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::InvalidRequest(_)));
    }

    #[tokio::test]
    async fn query_budget_stops_before_sending() {
        let h = harness(Budget::new(1, 100)).await;
        Mock::given(method("POST"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(search_response()))
            .expect(1)
            .mount(&h.server)
            .await;

        h.provider
            .search(SearchRequest::new("first"))
            .await
            .unwrap();

        let err = h
            .provider
            .search(SearchRequest::new("second"))
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::Budget(_)));
        assert_eq!(h.server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn retries_rate_limited_requests() {
        let h = harness(Budget::new(10, 100)).await;
        Mock::given(method("POST"))
            .and(path("/search"))
            .respond_with(FirstRateLimited {
                calls: AtomicUsize::new(0),
            })
            .expect(2)
            .mount(&h.server)
            .await;

        let result = h
            .provider
            .search(SearchRequest::new("retry me"))
            .await
            .unwrap();
        assert_eq!(result.data.results[0].title, "Example");
        assert_eq!(h.server.received_requests().await.unwrap().len(), 2);
    }
}
