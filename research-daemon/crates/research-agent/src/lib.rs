//! Budgeted, model-driven research agent.
//!
//! The daemon owns the loop. The model chooses one action at a time from a
//! constrained JSON schema; the daemon executes the action through the same
//! search/extract provider as the deterministic pipeline, records the full
//! transcript in SQLite, and enforces the job budget.

pub mod action;
pub mod prompt;
pub mod reddit;
pub mod runner;

pub use action::{AgentAction, PlanDraft, SubquestionDraft, SynthesisDraft};
pub use prompt::PROMPT_VERSION;
pub use reddit::{
    explicit_reddit_mention, opinion_intent, should_enable_reddit, should_preseed_reddit,
};
pub use runner::{AgentError, AgentOutcome, AgentRunner};
