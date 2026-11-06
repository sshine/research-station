use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            pub fn into_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self)
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }
    };
}

define_id!(JobId);
define_id!(PlanId);
define_id!(SubquestionId);
define_id!(QueryId);
define_id!(SourceId);
define_id!(PassageId);
define_id!(ClaimId);

/// How much retrieval and synthesis work a job may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Depth {
    Quick,
    #[default]
    Thorough,
    Exhaustive,
}

impl Depth {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quick => "quick",
            Self::Thorough => "thorough",
            Self::Exhaustive => "exhaustive",
        }
    }

    /// Default query allowance for a job at this depth.
    pub fn default_max_queries(self) -> u32 {
        match self {
            Self::Quick => 12,
            Self::Thorough => 40,
            Self::Exhaustive => 100,
        }
    }
}

/// Whether a job pauses for human plan review before collecting evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewPolicy {
    #[default]
    Automatic,
    Manual,
}

/// Which research engine runs a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Engine {
    /// The iterative, model-driven agent loop.
    #[default]
    Agent,
    /// The deterministic planner, collector, and extractive synthesizer.
    Deterministic,
}

impl Engine {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Deterministic => "deterministic",
        }
    }
}

/// Per-job resource limits. Reserved before work starts, never after.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobBudget {
    pub max_queries: u32,
    pub max_sources: u32,
    pub max_llm_tokens: u64,
    pub max_credits: u64,
    pub max_wall_seconds: u64,
    /// Maximum model action turns across the whole job.
    pub max_agent_rounds: u32,
}

impl JobBudget {
    pub fn for_depth(depth: Depth) -> Self {
        Self {
            max_queries: depth.default_max_queries(),
            ..Self::default()
        }
    }

    pub fn validate(&self) -> Result<(), RequestError> {
        if self.max_queries == 0 {
            return Err(RequestError::ZeroBudget("max_queries"));
        }
        if self.max_sources == 0 {
            return Err(RequestError::ZeroBudget("max_sources"));
        }
        if self.max_credits == 0 {
            return Err(RequestError::ZeroBudget("max_credits"));
        }
        if self.max_wall_seconds == 0 {
            return Err(RequestError::ZeroBudget("max_wall_seconds"));
        }
        if self.max_agent_rounds == 0 {
            return Err(RequestError::ZeroBudget("max_agent_rounds"));
        }
        Ok(())
    }

    pub fn wall_time(&self) -> Duration {
        Duration::from_secs(self.max_wall_seconds)
    }
}

impl Default for JobBudget {
    fn default() -> Self {
        Self {
            max_queries: Depth::default().default_max_queries(),
            max_sources: 60,
            max_llm_tokens: 200_000,
            max_credits: 100,
            max_wall_seconds: 900,
            max_agent_rounds: 24,
        }
    }
}

/// Everything the daemon needs to start a research job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchRequest {
    pub question: String,
    #[serde(default)]
    pub engine: Engine,
    #[serde(default)]
    pub depth: Depth,
    #[serde(default)]
    pub include_domains: Vec<String>,
    #[serde(default)]
    pub exclude_domains: Vec<String>,
    #[serde(default)]
    pub budget: JobBudget,
    #[serde(default)]
    pub review: ReviewPolicy,
    /// Bundle directory requested by the caller. The service allocates one when
    /// this is absent.
    #[serde(default)]
    pub output_dir: Option<PathBuf>,
}

impl ResearchRequest {
    pub fn new(question: impl Into<String>) -> Self {
        Self {
            question: question.into(),
            engine: Engine::default(),
            depth: Depth::default(),
            include_domains: Vec::new(),
            exclude_domains: Vec::new(),
            budget: JobBudget::default(),
            review: ReviewPolicy::default(),
            output_dir: None,
        }
    }

    pub fn validate(&self) -> Result<(), RequestError> {
        if self.question.trim().is_empty() {
            return Err(RequestError::EmptyQuestion);
        }
        if self.include_domains.len() > 20 || self.exclude_domains.len() > 20 {
            return Err(RequestError::TooManyDomains);
        }
        self.budget.validate()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("question must not be empty")]
    EmptyQuestion,
    #[error("at most 20 included and 20 excluded domains are allowed")]
    TooManyDomains,
    #[error("budget field {0} must be greater than zero")]
    ZeroBudget(&'static str),
}

/// Durable job lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobStatus {
    Queued,
    Planning,
    AwaitingReview,
    Running,
    Synthesizing,
    Auditing,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

impl JobStatus {
    pub const ALL: [Self; 10] = [
        Self::Queued,
        Self::Planning,
        Self::AwaitingReview,
        Self::Running,
        Self::Synthesizing,
        Self::Auditing,
        Self::Completed,
        Self::Failed,
        Self::Cancelled,
        Self::Interrupted,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Planning => "planning",
            Self::AwaitingReview => "awaiting-review",
            Self::Running => "running",
            Self::Synthesizing => "synthesizing",
            Self::Auditing => "auditing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    /// Completed and cancelled jobs are final. Failed and interrupted jobs may
    /// be resumed deliberately.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }

    /// Whether a worker may currently hold a lease for this status.
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Planning
                | Self::AwaitingReview
                | Self::Running
                | Self::Synthesizing
                | Self::Auditing
        )
    }

    pub fn is_resumable(self) -> bool {
        matches!(self, Self::Interrupted | Self::Failed)
    }

    /// Validate the state machine before touching storage.
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }

        match self {
            Self::Queued => matches!(next, Self::Planning | Self::Cancelled | Self::Failed),
            Self::Planning => matches!(
                next,
                Self::AwaitingReview
                    | Self::Running
                    | Self::Failed
                    | Self::Cancelled
                    | Self::Interrupted
            ),
            Self::AwaitingReview => matches!(
                next,
                Self::Queued | Self::Running | Self::Failed | Self::Cancelled | Self::Interrupted
            ),
            Self::Running => matches!(
                next,
                Self::Synthesizing | Self::Failed | Self::Cancelled | Self::Interrupted
            ),
            Self::Synthesizing => matches!(
                next,
                Self::Auditing | Self::Failed | Self::Cancelled | Self::Interrupted
            ),
            Self::Auditing => matches!(
                next,
                Self::Completed | Self::Failed | Self::Cancelled | Self::Interrupted
            ),
            Self::Interrupted | Self::Failed => {
                matches!(next, Self::Queued | Self::Cancelled | Self::Failed)
            }
            Self::Completed | Self::Cancelled => false,
        }
    }
}

impl fmt::Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown job status: {0}")]
pub struct ParseJobStatusError(pub String);

impl FromStr for JobStatus {
    type Err = ParseJobStatusError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_str() == value)
            .ok_or_else(|| ParseJobStatusError(value.to_owned()))
    }
}

/// A persisted research job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub request: ResearchRequest,
    pub status: JobStatus,
    pub output_dir: PathBuf,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub idempotency_key: Option<String>,
    pub cancel_requested: bool,
}

impl Job {
    pub fn is_cancellation_requested(&self) -> bool {
        self.cancel_requested
    }
}

/// One transition in a job's history. The daemon writes events; clients read
/// them and the future SSE endpoint replays them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobEvent {
    pub at: DateTime<Utc>,
    pub status: JobStatus,
    pub detail: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_time_ordered_and_roundtrip() {
        let first = JobId::new();
        let second = JobId::new();
        assert!(first < second);

        let text = first.to_string();
        assert_eq!(text.parse::<JobId>().unwrap(), first);
    }

    #[test]
    fn terminal_states_do_not_transition() {
        for status in [JobStatus::Completed, JobStatus::Cancelled] {
            for next in JobStatus::ALL.into_iter().filter(|next| *next != status) {
                assert!(!status.can_transition_to(next));
            }
        }
    }

    #[test]
    fn happy_path_transitions_are_allowed() {
        let path = [
            JobStatus::Queued,
            JobStatus::Planning,
            JobStatus::Running,
            JobStatus::Synthesizing,
            JobStatus::Auditing,
            JobStatus::Completed,
        ];

        for pair in path.windows(2) {
            assert!(
                pair[0].can_transition_to(pair[1]),
                "{} -> {} should be allowed",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn interrupted_and_failed_jobs_can_resume() {
        for status in [JobStatus::Interrupted, JobStatus::Failed] {
            assert!(status.can_transition_to(JobStatus::Queued));
            assert!(status.is_resumable());
        }
    }

    #[test]
    fn status_roundtrips_through_text() {
        for status in JobStatus::ALL {
            assert_eq!(status.as_str().parse::<JobStatus>().unwrap(), status);
        }
        assert!("done".parse::<JobStatus>().is_err());
    }

    #[test]
    fn empty_questions_and_zero_budgets_are_rejected() {
        assert!(matches!(
            ResearchRequest::new("  ").validate(),
            Err(RequestError::EmptyQuestion)
        ));

        let budget = JobBudget {
            max_queries: 0,
            ..JobBudget::default()
        };
        assert!(budget.validate().is_err());
    }

    #[test]
    fn depth_sets_the_query_allowance() {
        assert_eq!(JobBudget::for_depth(Depth::Quick).max_queries, 12);
        assert_eq!(JobBudget::for_depth(Depth::Thorough).max_queries, 40);
        assert_eq!(JobBudget::for_depth(Depth::Exhaustive).max_queries, 100);
    }

    #[test]
    fn agent_is_the_default_engine() {
        assert_eq!(ResearchRequest::new("question").engine, Engine::Agent);
        assert_eq!(Engine::default().as_str(), "agent");
        assert_eq!(JobBudget::default().max_agent_rounds, 24);
    }
}
