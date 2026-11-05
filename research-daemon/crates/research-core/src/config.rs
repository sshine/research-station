use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Top-level daemon configuration.
///
/// The API key itself is never stored in this file; `api_key_env` names the
/// environment variable that carries it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub provider: ProviderConfig,
    pub store: StoreConfig,
    pub budget: BudgetConfig,
    pub jobs: JobsConfig,
}

impl Config {
    /// Load a TOML config file, or return defaults when no path is supplied.
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        match path {
            Some(path) => {
                let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
                    path: path.display().to_string(),
                    source,
                })?;
                toml::from_str(&text).map_err(|source| ConfigError::Parse {
                    path: path.display().to_string(),
                    source,
                })
            }
            None => Ok(Self::default()),
        }
    }

    /// Read the provider key from the configured environment variable.
    pub fn api_key(&self) -> Result<String, ConfigError> {
        let name = &self.provider.tavily.api_key_env;
        match std::env::var(name) {
            Ok(value) if !value.trim().is_empty() => Ok(value),
            _ => Err(ConfigError::MissingEnv(name.clone())),
        }
    }

    /// Find a config file from an explicit path, the environment, a project
    /// config, or the XDG config directory.
    pub fn discover(explicit: Option<PathBuf>) -> Option<PathBuf> {
        if explicit.is_some() {
            return explicit;
        }

        if let Ok(path) = std::env::var("RESEARCHD_CONFIG") {
            return Some(PathBuf::from(path));
        }

        for candidate in [
            PathBuf::from("research-daemon/config/researchd.toml"),
            PathBuf::from("config/researchd.toml"),
        ] {
            if candidate.exists() {
                return Some(candidate);
            }
        }

        dirs::config_dir()
            .map(|dir| dir.join("research-station/researchd.toml"))
            .filter(|path| path.exists())
    }

    /// SQLite cache location, defaulting to the XDG cache directory.
    ///
    /// Jobs and cache metadata share one database. `cache_path` is the older
    /// name for the same file; `database_path` takes precedence when both are
    /// set.
    pub fn cache_path(&self) -> PathBuf {
        self.database_path()
    }

    /// SQLite database for jobs, plans, evidence indexes, usage, and cache
    /// metadata.
    pub fn database_path(&self) -> PathBuf {
        self.store
            .database_path
            .clone()
            .or_else(|| self.store.cache_path.clone())
            .unwrap_or_else(|| default_cache_dir().join("cache.sqlite"))
    }

    /// Content-addressed blob directory, defaulting to the XDG cache directory.
    pub fn blob_dir(&self) -> PathBuf {
        self.store
            .blob_dir
            .clone()
            .unwrap_or_else(|| default_cache_dir().join("blobs"))
    }
}

fn default_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("research-station")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderConfig {
    pub tavily: TavilyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TavilyConfig {
    pub api_key_env: String,
    pub base_url: String,
    pub requests_per_minute: u32,
    pub burst: u32,
    pub max_concurrency: usize,
    pub max_retries: u32,
    pub timeout_seconds: u64,
    pub cache_ttl_seconds: u64,
    /// Optional path to `secretspec.toml`. When omitted, SecretSpec searches
    /// upward from the working directory, and `SECRETSPEC_FILE` is honoured.
    pub secretspec_file: Option<PathBuf>,
}

impl Default for TavilyConfig {
    fn default() -> Self {
        Self {
            api_key_env: "TAVILY_API_KEY".to_owned(),
            base_url: "https://api.tavily.com".to_owned(),
            requests_per_minute: 60,
            burst: 10,
            max_concurrency: 4,
            max_retries: 4,
            timeout_seconds: 60,
            cache_ttl_seconds: 21_600,
            secretspec_file: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StoreConfig {
    pub database_path: Option<PathBuf>,
    pub cache_path: Option<PathBuf>,
    pub blob_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BudgetConfig {
    pub max_queries: u32,
    pub max_credits: u64,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            max_queries: 100,
            max_credits: 1_000,
        }
    }
}

/// Queue and worker defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct JobsConfig {
    /// How long a worker owns a claimed job before another worker may reap it.
    pub lease_seconds: u64,
    /// Global cap on jobs executing at once.
    pub max_concurrent_jobs: u32,
    /// How often the service looks for expired leases.
    pub reap_interval_seconds: u64,
    /// How long an idle worker waits before polling the queue again.
    pub poll_interval_seconds: u64,
}

impl Default for JobsConfig {
    fn default() -> Self {
        Self {
            lease_seconds: 60,
            max_concurrent_jobs: 1,
            reap_interval_seconds: 30,
            poll_interval_seconds: 2,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("missing environment variable {0}")]
    MissingEnv(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let config = Config::default();
        assert_eq!(config.provider.tavily.base_url, "https://api.tavily.com");
        assert_eq!(config.provider.tavily.requests_per_minute, 60);
        assert!(
            config
                .cache_path()
                .ends_with("research-station/cache.sqlite")
        );
    }
}
