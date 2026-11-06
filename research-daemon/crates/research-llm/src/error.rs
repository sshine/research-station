use std::time::Duration;

/// Errors surfaced by a model provider or its retry wrapper.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("invalid model request: {0}")]
    InvalidRequest(String),
    #[error("model provider rate limited: {message}")]
    RateLimited {
        message: String,
        retry_after: Option<Duration>,
    },
    #[error("transient model provider error: {0}")]
    Transient(String),
    #[error("model API error ({status}): {message}")]
    Api { status: u16, message: String },
    #[error("transport error: {0}")]
    Transport(String),
    #[error("failed to decode model response: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("empty model response: {0}")]
    EmptyResponse(String),
    #[error("invalid model configuration: {0}")]
    Config(String),
}

impl LlmError {
    /// Whether retrying the same request could succeed.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::RateLimited { .. } | Self::Transient(_))
    }

    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimited { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_classification() {
        assert!(
            LlmError::RateLimited {
                message: "slow down".to_owned(),
                retry_after: Some(Duration::from_secs(1)),
            }
            .is_retryable()
        );
        assert!(LlmError::Transient("502".to_owned()).is_retryable());
        assert!(!LlmError::Auth("bad key".to_owned()).is_retryable());
        assert!(!LlmError::InvalidRequest("bad request".to_owned()).is_retryable());
    }
}
