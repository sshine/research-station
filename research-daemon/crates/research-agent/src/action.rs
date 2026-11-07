use research_core::{RedditSearchMode, RedditSort, SearchTopic, SourceClass, TimeRange};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// One model-chosen action in the per-subquestion loop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentAction {
    Search {
        query: String,
        #[serde(default)]
        max_results: Option<u8>,
        #[serde(default)]
        topic: Option<SearchTopic>,
        #[serde(default)]
        time_range: Option<TimeRange>,
    },
    Extract {
        urls: Vec<String>,
        #[serde(default)]
        query: Option<String>,
    },
    RedditSearch {
        query: String,
        #[serde(default)]
        subreddit: Option<String>,
        #[serde(default)]
        sort: Option<RedditSort>,
        #[serde(default)]
        mode: Option<RedditSearchMode>,
        #[serde(default)]
        time_range: Option<TimeRange>,
        #[serde(default)]
        max_results: Option<u8>,
        #[serde(default)]
        min_score: Option<u32>,
        #[serde(default)]
        include_comments: Option<bool>,
        #[serde(default)]
        comment_limit: Option<u8>,
    },
    ReadSource {
        source_id: String,
    },
    Finish {
        satisfied: bool,
        reason: String,
    },
}

impl AgentAction {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Search { .. } => "search",
            Self::Extract { .. } => "extract",
            Self::RedditSearch { .. } => "reddit_search",
            Self::ReadSource { .. } => "read_source",
            Self::Finish { .. } => "finish",
        }
    }
}

/// The model's plan before the daemon assigns stable IDs, timestamps, and the
/// job's actual constraints and budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanDraft {
    pub sharpened_topic: String,
    pub subquestions: Vec<SubquestionDraft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubquestionDraft {
    pub question: String,
    pub source_class: SourceClass,
    pub success_criterion: String,
    #[serde(default)]
    pub query_seeds: Vec<String>,
}

/// The model's synthesis output. `subquestion` and `source` are 1-based indices
/// into the ledger shown in the prompt, which avoids asking the model to copy
/// UUIDs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SynthesisDraft {
    #[serde(default)]
    pub claims: Vec<ClaimDraft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaimDraft {
    pub subquestion: usize,
    pub text: String,
    #[serde(default)]
    pub evidence: Vec<EvidenceDraft>,
    #[serde(default)]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceDraft {
    pub source: usize,
    pub quote: String,
}

/// Parse a JSON object from a model response, tolerating markdown fences and a
/// short explanation before or after the object.
pub fn parse_json_object<T: DeserializeOwned>(content: &str) -> Result<T, serde_json::Error> {
    let trimmed = content.trim();
    let start = trimmed.find('{');
    let end = trimmed.rfind('}');
    let candidate = match (start, end) {
        (Some(start), Some(end)) if start <= end => &trimmed[start..=end],
        _ => trimmed,
    };
    serde_json::from_str(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_actions() {
        let search: AgentAction =
            parse_json_object(r#"{"action":"search","query":"kubernetes auth"}"#).unwrap();
        assert!(matches!(search, AgentAction::Search { .. }));

        let finish: AgentAction =
            parse_json_object(r#"{"action":"finish","satisfied":true,"reason":"enough"}"#).unwrap();
        assert!(matches!(
            finish,
            AgentAction::Finish {
                satisfied: true,
                ..
            }
        ));

        let reddit: AgentAction = parse_json_object(
            r#"{"action":"reddit_search","query":"framework problems","include_comments":true}"#,
        )
        .unwrap();
        assert!(matches!(reddit, AgentAction::RedditSearch { .. }));
        assert_eq!(reddit.kind(), "reddit_search");
    }

    #[test]
    fn parses_json_inside_fences() {
        let draft: PlanDraft = parse_json_object(
            r#"```json
            {"sharpened_topic":"topic","subquestions":[
              {"question":"q","source_class":"primary","success_criterion":"one source"}
            ]}
            ```"#,
        )
        .unwrap();
        assert_eq!(draft.subquestions.len(), 1);
    }

    #[test]
    fn rejects_non_json() {
        assert!(parse_json_object::<AgentAction>("not json").is_err());
    }
}
