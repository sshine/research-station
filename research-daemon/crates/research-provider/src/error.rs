use std::time::Duration;

use crate::budget::BudgetExceeded;
use research_store::StoreError;

/// Errors surfaced by a search provider or its operational wrapper.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("rate limited: {message}")]
    RateLimited {
        message: String,
        retry_after: Option<Duration>,
    },
    #[error("quota or plan limit exceeded: {0}")]
    QuotaExceeded(String),
    #[error("provider capacity exceeded: {0}")]
    CapacityExceeded(String),
    #[error("transient provider error: {0}")]
    Transient(String),
    #[error("provider API error ({status}): {message}")]
    Api { status: u16, message: String },
    #[error("transport error: {0}")]
    Transport(String),
    #[error("failed to decode response: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("budget exceeded: {0}")]
    Budget(#[from] BudgetExceeded),
}

impl ProviderError {
    /// Whether retrying the same request could succeed.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimited { .. } | Self::Transient(_) | Self::CapacityExceeded(_)
        )
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
            ProviderError::RateLimited {
                message: "slow down".into(),
                retry_after: Some(Duration::from_secs(1)),
            }
            .is_retryable()
        );
        assert!(ProviderError::Transient("502".into()).is_retryable());
        assert!(ProviderError::CapacityExceeded("busy".into()).is_retryable());
        assert!(!ProviderError::Auth("bad key".into()).is_retryable());
        assert!(!ProviderError::InvalidRequest("bad query".into()).is_retryable());
    }
}
