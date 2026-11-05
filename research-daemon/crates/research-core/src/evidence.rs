use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::job::{ClaimId, JobBudget, PassageId, PlanId, QueryId, SourceId, SubquestionId};

/// What kind of source a subquestion is expected to need.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SourceClass {
    Primary,
    #[default]
    Secondary,
    Community,
    News,
    Reference,
}

impl SourceClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Community => "community",
            Self::News => "news",
            Self::Reference => "reference",
        }
    }
}

/// An immutable, versioned research plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub id: PlanId,
    pub version: u32,
    pub raw_topic: String,
    pub sharpened_topic: String,
    pub subquestions: Vec<Subquestion>,
    pub constraints: PlanConstraints,
    pub budget: JobBudget,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subquestion {
    pub id: SubquestionId,
    pub question: String,
    pub source_class: SourceClass,
    pub query_seeds: Vec<String>,
    pub success_criterion: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanConstraints {
    pub include_domains: Vec<String>,
    pub exclude_domains: Vec<String>,
}

/// One provider call, recorded for reproducibility and cost accounting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryRecord {
    pub id: QueryId,
    pub subquestion_id: SubquestionId,
    pub provider: String,
    pub text: String,
    pub at: DateTime<Utc>,
    pub status: String,
    pub result_count: usize,
    pub latency_ms: u64,
    pub credits: Option<u64>,
    pub raw_hash: Option<String>,
    pub cache_hit: bool,
}

/// A source accepted into the evidence ledger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceRecord {
    pub id: SourceId,
    pub subquestion_id: SubquestionId,
    pub url: String,
    pub canonical_url: String,
    pub title: String,
    pub publisher: String,
    pub published_date: Option<String>,
    pub retrieved_at: DateTime<Utc>,
    pub source_type: SourceClass,
    pub content_hash: String,
    pub quality_rationale: String,
    pub independence_group: String,
    /// Path relative to the bundle root.
    pub content_path: String,
    pub excerpt: String,
}

/// A quote tied to a claim. The quote must be an exact substring of the stored
/// source content for the claim to survive the audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub source_id: SourceId,
    pub passage_id: PassageId,
    pub quote: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SupportStatus {
    Supported,
    PartiallySupported,
    Unsupported,
}

impl SupportStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::PartiallySupported => "partially-supported",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Claim {
    pub id: ClaimId,
    pub subquestion_id: SubquestionId,
    pub text: String,
    pub evidence: Vec<Evidence>,
    pub confidence: f64,
    pub support: SupportStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubquestionCoverage {
    pub subquestion_id: SubquestionId,
    pub claim_count: usize,
    pub source_count: usize,
}

/// Output of synthesis. Rendering is separate so the same claims can be
/// audited before anything is published.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Synthesis {
    pub claims: Vec<Claim>,
    pub coverage: Vec<SubquestionCoverage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditFinding {
    pub claim_id: ClaimId,
    pub source_id: SourceId,
    pub quote: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditReport {
    pub total_claims: usize,
    pub supported_claims: usize,
    pub covered_subquestions: usize,
    pub total_subquestions: usize,
    pub findings: Vec<AuditFinding>,
    pub passed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_roundtrip_through_json() {
        let plan = Plan {
            id: PlanId::new(),
            version: 1,
            raw_topic: "topic".to_owned(),
            sharpened_topic: "sharpened topic".to_owned(),
            subquestions: vec![Subquestion {
                id: SubquestionId::new(),
                question: "What is the topic?".to_owned(),
                source_class: SourceClass::Reference,
                query_seeds: vec!["topic overview".to_owned()],
                success_criterion: "Two sources".to_owned(),
            }],
            constraints: PlanConstraints::default(),
            budget: JobBudget::default(),
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&plan).unwrap();
        let decoded: Plan = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, plan);
    }
}
