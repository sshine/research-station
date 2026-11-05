//! Deterministic Phase 2 research pipeline.
//!
//! A rule-based planner produces a versioned plan. The collector turns query
//! seeds into provider calls and source records. Synthesis quotes stored
//! passages, the auditor verifies every quote against the stored text, and the
//! bundle writer produces the durable `research/` artifact.

pub mod audit;
pub mod bundle;
pub mod canonical;
pub mod collect;
pub mod plan;
pub mod render;
pub mod synth;

pub use audit::audit;
pub use bundle::{Bundle, NoteFile, Progress, RunMetadata, RunUsage};
pub use canonical::canonicalize_url;
pub use collect::{CollectedSubquestion, CollectionLimits, Collector};
pub use plan::RuleBasedPlanner;
pub use render::{render_audit, render_brief, render_plan, render_report};
pub use synth::synthesize;

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("provider error: {0}")]
    Provider(#[from] research_provider::ProviderError),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    #[error("bundle error: {0}")]
    Bundle(String),
}
