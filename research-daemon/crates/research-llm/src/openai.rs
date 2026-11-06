use std::time::{Duration, Instant};

use async_trait::async_trait;
use reqwest::StatusCode;
use reqwest::header::{CONTENT_TYPE, HeaderMap, RETRY_AFTER};
use research_core::LlmConfig;
use serde::{Deserialize, Serialize};

use crate::{LlmError, LlmMessage, LlmProvider, LlmRequest, LlmResponse, LlmUsage, ResponseFormat};

/// Connection settings for an OpenAI-compatible chat-completions endpoint.
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleConfig {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    /// Stable routing/caching identity for OpenCode Go. Generated when omitted.
    pub session_id: Option<String>,
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub temperature: f64,
    pub max_output_tokens: u32,
}

impl From<&LlmConfig> for OpenAiCompatibleConfig {
    fn from(config: &LlmConfig) -> Self {
        Self {
            provider: config.provider.clone(),
            base_url: config.base_url.clone(),
            model: config.model.clone(),
            session_id: config.session_id.clone(),
            timeout_seconds: config.timeout_seconds,
            max_retries: config.max_retries,
            temperature: config.temperature,
            max_output_tokens: config.max_output_tokens,
        }
    }
}

/// OpenAI-compatible client used for OpenCode Go and similar endpoints.
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    session_id: String,
    config: OpenAiCompatibleConfig,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        config: OpenAiCompatibleConfig,
        api_key: impl Into<String>,
    ) -> Result<Self, LlmError> {
        let base_url = config.base_url.trim().trim_end_matches('/').to_owned();
        if base_url.is_empty() {
            return Err(LlmError::Config(
                "llm.base_url must not be empty".to_owned(),
            ));
        }
        if config.model.trim().is_empty() {
            return Err(LlmError::Config("llm.model must not be empty".to_owned()));
        }

        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(LlmError::Config(
                "the model API key must not be empty".to_owned(),
            ));
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds.max(1)))
            .user_agent(concat!("research-station/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|err| LlmError::Transport(err.to_string()))?;

        let session_id = config
            .session_id
            .clone()
            .filter(|session| !session.trim().is_empty())
            .unwrap_or_else(default_session_id);

        Ok(Self {
            client,
            base_url,
            api_key,
            session_id,
            config,
        })
    }

    async fn post_with_retry(&self, body: &[u8]) -> Result<Vec<u8>, LlmError> {
        let mut attempt = 0u32;

        loop {
            match self.send_once(body).await {
                Ok(bytes) => return Ok(bytes),
                Err(err) if err.is_retryable() && attempt < self.config.max_retries => {
                    let delay = err.retry_after().unwrap_or_else(|| backoff(attempt));
                    tracing::warn!(
                        attempt = attempt + 1,
                        max_retries = self.config.max_retries,
                        delay_ms = delay.as_millis(),
                        error = %err,
                        "retrying model request"
                    );
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
                Err(err) => return Err(err),
            }
        }
    }

    async fn send_once(&self, body: &[u8]) -> Result<Vec<u8>, LlmError> {
        let url = format!("{}/chat/completions", self.base_url);
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .header(CONTENT_TYPE, "application/json")
            .header("x-opencode-session", &self.session_id)
            .body(body.to_vec())
            .send()
            .await
            .map_err(|err| LlmError::Transport(err.to_string()))?;

        let status = response.status();
        if status.is_success() {
            return response
                .bytes()
                .await
                .map(|bytes| bytes.to_vec())
                .map_err(|err| LlmError::Transport(err.to_string()));
        }

        let retry_after = parse_retry_after(response.headers());
        let message = response.text().await.unwrap_or_default();
        Err(classify(status, message, retry_after))
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    fn id(&self) -> &str {
        &self.config.provider
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        if request.messages.is_empty() {
            return Err(LlmError::InvalidRequest(
                "at least one message is required".to_owned(),
            ));
        }

        let body = ChatRequest {
            model: &self.config.model,
            messages: &request.messages,
            temperature: request.temperature.unwrap_or(self.config.temperature),
            max_tokens: request.max_tokens.unwrap_or(self.config.max_output_tokens),
            response_format: request.response_format.map(ResponseFormatBody::from),
        };
        let body = serde_json::to_vec(&body)?;
        let started = Instant::now();
        let bytes = self.post_with_retry(&body).await?;
        let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let response: ChatResponse = serde_json::from_slice(&bytes)?;

        let choice =
            response.choices.into_iter().next().ok_or_else(|| {
                LlmError::EmptyResponse("response contained no choices".to_owned())
            })?;

        let content = choice.message.content.unwrap_or_default();
        let reasoning = choice
            .message
            .reasoning_content
            .or(choice.message.reasoning);
        if content.trim().is_empty() && reasoning.as_deref().is_none_or(str::is_empty) {
            return Err(LlmError::EmptyResponse(
                "response contained neither content nor reasoning".to_owned(),
            ));
        }

        let usage = response.usage.unwrap_or_default().into_usage();

        Ok(LlmResponse {
            provider: self.config.provider.clone(),
            model: response.model.unwrap_or_else(|| self.config.model.clone()),
            content,
            reasoning,
            finish_reason: choice.finish_reason,
            usage,
            request_id: response.id,
            latency_ms,
        })
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [LlmMessage],
    temperature: f64,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormatBody>,
}

#[derive(Serialize)]
struct ResponseFormatBody {
    #[serde(rename = "type")]
    kind: &'static str,
}

impl From<ResponseFormat> for ResponseFormatBody {
    fn from(format: ResponseFormat) -> Self {
        match format {
            ResponseFormat::JsonObject => Self {
                kind: "json_object",
            },
        }
    }
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: Option<RawUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawUsage {
    #[serde(default)]
    prompt_tokens: Option<u64>,
    #[serde(default)]
    completion_tokens: Option<u64>,
    #[serde(default)]
    total_tokens: Option<u64>,
    #[serde(default)]
    cost: Option<f64>,
    #[serde(default)]
    opencode: Option<OpenCodeUsage>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenCodeUsage {
    #[serde(default)]
    cost: Option<f64>,
    #[serde(default)]
    billing_source: Option<String>,
}

impl RawUsage {
    fn into_usage(self) -> LlmUsage {
        let prompt_tokens = self.prompt_tokens.unwrap_or_default();
        let completion_tokens = self.completion_tokens.unwrap_or_default();
        let total_tokens = self
            .total_tokens
            .unwrap_or(prompt_tokens.saturating_add(completion_tokens));
        let (cost_usd, billing_source) = match self.opencode {
            Some(opencode) => (opencode.cost.or(self.cost), opencode.billing_source),
            None => (self.cost, None),
        };

        LlmUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cost_usd,
            billing_source,
        }
    }
}

fn classify(status: StatusCode, message: String, retry_after: Option<Duration>) -> LlmError {
    let code = status.as_u16();
    match code {
        401 | 403 => LlmError::Auth(message),
        400 | 404 | 422 => LlmError::InvalidRequest(message),
        429 => LlmError::RateLimited {
            message,
            retry_after,
        },
        500..=599 => LlmError::Transient(format!("{code}: {message}")),
        _ => LlmError::Api {
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

fn default_session_id() -> String {
    format!(
        "research-station-{:016x}{:016x}",
        fastrand::u64(..),
        fastrand::u64(..)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
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
                    .set_body_json(serde_json::json!({"error": "slow down"}))
            } else {
                ResponseTemplate::new(200).set_body_json(completion_response())
            }
        }
    }

    async fn harness() -> (OpenAiCompatibleProvider, MockServer) {
        let server = MockServer::start().await;
        let config = OpenAiCompatibleConfig {
            provider: "opencode-go".to_owned(),
            base_url: server.uri(),
            model: "deepseek-v4.1-flash".to_owned(),
            session_id: Some("test-session".to_owned()),
            timeout_seconds: 5,
            max_retries: 1,
            temperature: 0.2,
            max_output_tokens: 128,
        };
        let provider = OpenAiCompatibleProvider::new(config, "test-key").unwrap();
        (provider, server)
    }

    fn completion_response() -> serde_json::Value {
        serde_json::json!({
            "id": "req-1",
            "model": "deepseek-v4.1-flash",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "hello",
                    "reasoning_content": "thinking"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 12,
                "completion_tokens": 3,
                "total_tokens": 15,
                "opencode": {
                    "cost": 0.0001,
                    "billing_source": "balance"
                }
            }
        })
    }

    #[tokio::test]
    async fn completion_parses_content_usage_and_request() {
        let (provider, server) = harness().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(header("authorization", "Bearer test-key"))
            .and(header("x-opencode-session", "test-session"))
            .respond_with(ResponseTemplate::new(200).set_body_json(completion_response()))
            .expect(1)
            .mount(&server)
            .await;

        let request = LlmRequest::new(vec![
            LlmMessage::system("be terse"),
            LlmMessage::user("say hello"),
        ])
        .with_temperature(0.1)
        .with_max_tokens(32)
        .with_json_mode();

        let response = provider.complete(request).await.unwrap();
        assert_eq!(response.provider, "opencode-go");
        assert_eq!(response.model, "deepseek-v4.1-flash");
        assert_eq!(response.content, "hello");
        assert_eq!(response.reasoning.as_deref(), Some("thinking"));
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
        assert_eq!(response.request_id.as_deref(), Some("req-1"));
        assert_eq!(response.usage.prompt_tokens, 12);
        assert_eq!(response.usage.completion_tokens, 3);
        assert_eq!(response.usage.total_tokens, 15);
        assert_eq!(response.usage.cost_usd, Some(0.0001));
        assert_eq!(response.usage.billing_source.as_deref(), Some("balance"));

        let sent = server.received_requests().await.unwrap();
        assert_eq!(sent.len(), 1);
        let body: serde_json::Value = serde_json::from_slice(&sent[0].body).unwrap();
        assert_eq!(body["model"], "deepseek-v4.1-flash");
        assert_eq!(body["temperature"], 0.1);
        assert_eq!(body["max_tokens"], 32);
        assert_eq!(body["response_format"]["type"], "json_object");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "be terse");
        assert_eq!(body["messages"][1]["role"], "user");
    }

    #[tokio::test]
    async fn retries_rate_limited_requests() {
        let (provider, server) = harness().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(FirstRateLimited {
                calls: AtomicUsize::new(0),
            })
            .expect(2)
            .mount(&server)
            .await;

        let response = provider
            .complete(LlmRequest::new(vec![LlmMessage::user("retry me")]))
            .await
            .unwrap();
        assert_eq!(response.content, "hello");
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn auth_errors_are_not_retried() {
        let (provider, server) = harness().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(401).set_body_string("bad key"))
            .expect(1)
            .mount(&server)
            .await;

        let error = provider
            .complete(LlmRequest::new(vec![LlmMessage::user("hello")]))
            .await
            .unwrap_err();
        assert!(matches!(error, LlmError::Auth(_)));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[test]
    fn rejects_invalid_configuration() {
        let mut config = OpenAiCompatibleConfig {
            provider: "opencode-go".to_owned(),
            base_url: String::new(),
            model: "deepseek-v4.1-flash".to_owned(),
            session_id: None,
            timeout_seconds: 5,
            max_retries: 1,
            temperature: 0.2,
            max_output_tokens: 128,
        };
        assert!(matches!(
            OpenAiCompatibleProvider::new(config.clone(), "key"),
            Err(LlmError::Config(_))
        ));

        config.base_url = "https://example.com/v1".to_owned();
        config.model.clear();
        assert!(matches!(
            OpenAiCompatibleProvider::new(config, "key"),
            Err(LlmError::Config(_))
        ));
    }
}
