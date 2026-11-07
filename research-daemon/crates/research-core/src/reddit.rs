use serde::{Deserialize, Serialize};

use crate::TimeRange;

/// How aggressively the daemon should use Reddit for a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum RedditMode {
    /// Enable Reddit for explicit mentions, opinion intent, or community
    /// subquestions.
    #[default]
    Auto,
    /// Make the tool available and pre-seed every subquestion, up to the
    /// configured per-job cap.
    Always,
    /// Disable Reddit entirely for this job.
    Never,
}

impl RedditMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Always => "always",
            Self::Never => "never",
        }
    }
}

/// How Reddit search results should be ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum RedditSort {
    /// Reddit's relevance ranking. The safest default for discovery.
    #[default]
    Relevance,
    Hot,
    New,
    /// Score ranking. Reddit's `top` search matches loosely; use it only with
    /// a quoted phrase or a subreddit scope.
    Top,
    Comments,
}

impl RedditSort {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Hot => "hot",
            Self::New => "new",
            Self::Top => "top",
            Self::Comments => "comments",
        }
    }
}

/// Which Reddit search tab to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum RedditSearchMode {
    /// Search post titles and selftext.
    #[default]
    Posts,
    /// Search comment bodies. Reddit returns the parent posts, not the
    /// matching comments.
    Comments,
}

impl RedditSearchMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Posts => "posts",
            Self::Comments => "comments",
        }
    }
}

/// A provider-neutral Reddit search request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedditSearchRequest {
    pub query: String,
    #[serde(default)]
    pub subreddit: Option<String>,
    #[serde(default)]
    pub sort: RedditSort,
    #[serde(default)]
    pub mode: RedditSearchMode,
    #[serde(default)]
    pub time_range: Option<TimeRange>,
    #[serde(default)]
    pub max_results: Option<u8>,
    #[serde(default)]
    pub min_score: Option<u32>,
    /// `None` means use the provider default.
    #[serde(default)]
    pub include_comments: Option<bool>,
    #[serde(default)]
    pub comment_limit: Option<u8>,
}

impl RedditSearchRequest {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            subreddit: None,
            sort: RedditSort::Relevance,
            mode: RedditSearchMode::Posts,
            time_range: None,
            max_results: Some(25),
            min_score: None,
            include_comments: None,
            comment_limit: None,
        }
    }

    pub fn with_subreddit(mut self, subreddit: impl Into<String>) -> Self {
        self.subreddit = Some(subreddit.into());
        self
    }

    pub fn with_comments(mut self, limit: u8) -> Self {
        self.include_comments = Some(true);
        self.comment_limit = Some(limit);
        self
    }
}

/// Fetch one thread and its comments by post id or Reddit URL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedditThreadRequest {
    pub id_or_url: String,
    #[serde(default)]
    pub limit: Option<u8>,
    #[serde(default)]
    pub sort: RedditSort,
}

impl RedditThreadRequest {
    pub fn new(id_or_url: impl Into<String>) -> Self {
        Self {
            id_or_url: id_or_url.into(),
            limit: Some(100),
            sort: RedditSort::Top,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_and_sorts_roundtrip() {
        for mode in [RedditMode::Auto, RedditMode::Always, RedditMode::Never] {
            let text = serde_json::to_string(&mode).unwrap();
            assert_eq!(serde_json::from_str::<RedditMode>(&text).unwrap(), mode);
            assert!(!mode.as_str().is_empty());
        }
        for sort in [
            RedditSort::Relevance,
            RedditSort::Hot,
            RedditSort::New,
            RedditSort::Top,
            RedditSort::Comments,
        ] {
            let text = serde_json::to_string(&sort).unwrap();
            assert_eq!(serde_json::from_str::<RedditSort>(&text).unwrap(), sort);
            assert!(!sort.as_str().is_empty());
        }
    }

    #[test]
    fn request_builders_are_chainable() {
        let request = RedditSearchRequest::new("framework laptop")
            .with_subreddit("framework")
            .with_comments(20);
        assert_eq!(request.subreddit.as_deref(), Some("framework"));
        assert_eq!(request.include_comments, Some(true));
        assert_eq!(request.comment_limit, Some(20));
    }
}
