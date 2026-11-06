//! Minimal, provider-neutral LLM client for research-daemon.
//!
//! This crate deliberately avoids agent SDKs. It provides the small contract the
//! agent loop needs: chat messages, a completion request and response, token and
//! cost accounting, an OpenAI-compatible client for OpenCode Go, and a
//! deterministic fake for tests.

pub mod error;
pub mod fake;
pub mod openai;

pub use error::LlmError;
pub use fake::FakeLlm;
pub use openai::{OpenAiCompatibleConfig, OpenAiCompatibleProvider};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A chat role understood by OpenAI-compatible APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmRole {
    System,
    User,
    Assistant,
}

/// One message in a model conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: LlmRole,
    pub content: String,
}

impl LlmMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: LlmRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: LlmRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: LlmRole::Assistant,
            content: content.into(),
        }
    }
}

/// Requested output format. `JsonObject` maps to OpenAI's
/// `response_format: {"type": "json_object"}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseFormat {
    JsonObject,
}

/// A provider-neutral chat completion request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmRequest {
    pub messages: Vec<LlmMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
}

impl LlmRequest {
    pub fn new(messages: Vec<LlmMessage>) -> Self {
        Self {
            messages,
            temperature: None,
            max_tokens: None,
            response_format: None,
        }
    }

    pub fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_json_mode(mut self) -> Self {
        self.response_format = Some(ResponseFormat::JsonObject);
        self
    }
}

/// Token and cost usage reported by the model provider.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_source: Option<String>,
}

/// A completed model response plus the metadata needed for an agent trace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmResponse {
    pub provider: String,
    pub model: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    pub usage: LlmUsage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub latency_ms: u64,
}

/// Every model backend implements this trait. The agent loop never knows which
/// vendor is behind it.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn id(&self) -> &str;

    fn model(&self) -> &str;

    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError>;
}

/// Runtime dispatcher for the real provider and the deterministic fake.
#[derive(Debug, Clone)]
pub enum AnyLlm {
    OpenAi(OpenAiCompatibleProvider),
    Fake(FakeLlm),
}

#[async_trait]
impl LlmProvider for AnyLlm {
    fn id(&self) -> &str {
        match self {
            Self::OpenAi(provider) => provider.id(),
            Self::Fake(provider) => provider.id(),
        }
    }

    fn model(&self) -> &str {
        match self {
            Self::OpenAi(provider) => provider.model(),
            Self::Fake(provider) => provider.model(),
        }
    }

    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        match self {
            Self::OpenAi(provider) => provider.complete(request).await,
            Self::Fake(provider) => provider.complete(request).await,
        }
    }
}
