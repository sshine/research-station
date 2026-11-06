//! Durable storage for research-daemon.
//!
//! Provides the SQLite query cache, the content-addressed blob store, and the
//! durable job queue used by the service layer.

pub mod agent;
pub mod blobs;
pub mod cache;
pub mod jobs;

pub use agent::{AgentEvent, AgentRun, AgentStore};
pub use blobs::{BlobRef, BlobStore};
pub use cache::{CachedResponse, SqliteCache};
pub use jobs::{EnqueueOutcome, JobStore, ReapedJob};

use research_core::{JobId, JobStatus};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] sqlx::Error),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid blob hash: {0}")]
    InvalidHash(String),
    #[error("job {0} not found")]
    JobNotFound(JobId),
    #[error("invalid job transition from {from} to {to}")]
    InvalidTransition { from: JobStatus, to: JobStatus },
    #[error("job {id} is {status} and cannot be resumed")]
    NotResumable { id: JobId, status: JobStatus },
    #[error("job {id} is not leased by {owner}")]
    LeaseNotHeld { id: JobId, owner: String },
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("invalid stored data: {0}")]
    Data(String),
}
