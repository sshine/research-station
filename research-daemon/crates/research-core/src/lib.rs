//! Domain types shared by every research-daemon component.
//!
//! This crate deliberately has no HTTP, database, or provider knowledge.

pub mod config;
pub mod domain;
pub mod evidence;
pub mod job;
pub mod text;

pub use config::{
    BudgetConfig, Config, ConfigError, JobsConfig, LlmConfig, ProviderConfig, StoreConfig,
    TavilyConfig,
};
pub use domain::{
    ExtractDepth, ExtractFailure, ExtractFormat, ExtractRequest, ExtractResponse, ExtractResult,
    RawContentFormat, SearchDepth, SearchRequest, SearchResponse, SearchResult, SearchTopic,
    TimeRange, Usage,
};
pub use evidence::{
    AuditFinding, AuditReport, Claim, Evidence, Plan, PlanConstraints, QueryRecord, SourceClass,
    SourceRecord, Subquestion, SubquestionCoverage, SupportStatus, Synthesis,
};
pub use job::{
    ClaimId, Depth, Engine, Job, JobBudget, JobEvent, JobId, JobStatus, ParseJobStatusError,
    PassageId, PlanId, QueryId, RequestError, ResearchRequest, ReviewPolicy, SourceId,
    SubquestionId,
};
pub use text::slugify;
