use research_agent::AgentError;
use research_core::{ConfigError, JobId};
use research_llm::LlmError;
use research_pipeline::PipelineError;
use research_provider::ProviderError;
use research_store::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("configuration error: {0}")]
    Config(#[from] ConfigError),
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("pipeline error: {0}")]
    Pipeline(#[from] PipelineError),
    #[error("provider error: {0}")]
    Provider(#[from] ProviderError),
    #[error("llm error: {0}")]
    Llm(#[from] LlmError),
    #[error("agent error: {0}")]
    Agent(#[from] AgentError),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("job {0} was cancelled")]
    Cancelled(JobId),
    #[error("job {0} exceeded its wall-time budget")]
    Timeout(JobId),
    #[error("{0}")]
    Message(String),
}
