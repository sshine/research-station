use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// How much retrieval work Tavily should do for a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SearchDepth {
    Fast,
    UltraFast,
    #[default]
    Basic,
    Advanced,
}

impl SearchDepth {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::UltraFast => "ultra-fast",
            Self::Basic => "basic",
            Self::Advanced => "advanced",
        }
    }

    /// Tavily charges 2 credits for advanced search and 1 for the other depths.
    pub fn estimated_credits(self) -> u64 {
        match self {
            Self::Advanced => 2,
            _ => 1,
        }
    }
}

/// Tavily search topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SearchTopic {
    #[default]
    General,
    News,
    Finance,
}

impl SearchTopic {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::News => "news",
            Self::Finance => "finance",
        }
    }
}

/// Relative freshness window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimeRange {
    Day,
    Week,
    Month,
    Year,
}

impl TimeRange {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
        }
    }
}

/// Format requested for `raw_content` in search results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RawContentFormat {
    Markdown,
    Text,
}

impl RawContentFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Text => "text",
        }
    }
}

/// How deeply Tavily should extract a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ExtractDepth {
    #[default]
    Basic,
    Advanced,
}

impl ExtractDepth {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Advanced => "advanced",
        }
    }

    /// Tavily charges 1 credit per 5 successful URLs for basic, 2 for advanced.
    pub fn credits_per_five_urls(self) -> u64 {
        match self {
            Self::Basic => 1,
            Self::Advanced => 2,
        }
    }
}

/// Format requested for extracted content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ExtractFormat {
    #[default]
    Markdown,
    Text,
}

impl ExtractFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Text => "text",
        }
    }
}

/// Provider-neutral search request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub max_results: Option<u8>,
    #[serde(default)]
    pub search_depth: SearchDepth,
    #[serde(default)]
    pub topic: SearchTopic,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include_domains: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_domains: Vec<String>,
    #[serde(default)]
    pub time_range: Option<TimeRange>,
    #[serde(default)]
    pub start_date: Option<NaiveDate>,
    #[serde(default)]
    pub end_date: Option<NaiveDate>,
    #[serde(default)]
    pub include_raw_content: Option<RawContentFormat>,
    #[serde(default)]
    pub include_published_date: bool,
    #[serde(default)]
    pub filter_by_published_date: bool,
    #[serde(default)]
    pub country: Option<String>,
}

impl SearchRequest {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            max_results: Some(10),
            search_depth: SearchDepth::Basic,
            topic: SearchTopic::General,
            include_domains: Vec::new(),
            exclude_domains: Vec::new(),
            time_range: None,
            start_date: None,
            end_date: None,
            include_raw_content: None,
            include_published_date: false,
            filter_by_published_date: false,
            country: None,
        }
    }

    pub fn with_depth(mut self, depth: SearchDepth) -> Self {
        self.search_depth = depth;
        self
    }

    pub fn with_max_results(mut self, max_results: u8) -> Self {
        self.max_results = Some(max_results);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub raw_content: Option<String>,
    #[serde(default)]
    pub published_date: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResponse {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub answer: Option<String>,
    #[serde(default)]
    pub results: Vec<SearchResult>,
    #[serde(default)]
    pub response_time: Option<f64>,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

/// Provider-neutral extract request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractRequest {
    pub urls: Vec<String>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub extract_depth: ExtractDepth,
    #[serde(default)]
    pub format: ExtractFormat,
    #[serde(default)]
    pub include_images: bool,
    #[serde(default)]
    pub timeout_seconds: Option<f64>,
}

impl ExtractRequest {
    pub fn new(urls: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            urls: urls.into_iter().map(Into::into).collect(),
            query: None,
            extract_depth: ExtractDepth::Basic,
            format: ExtractFormat::Markdown,
            include_images: false,
            timeout_seconds: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractResult {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub raw_content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractFailure {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractResponse {
    #[serde(default)]
    pub results: Vec<ExtractResult>,
    #[serde(default)]
    pub failed_results: Vec<ExtractFailure>,
    #[serde(default)]
    pub response_time: Option<f64>,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub credits: Option<u64>,
}
