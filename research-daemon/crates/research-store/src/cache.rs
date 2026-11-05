use std::path::Path;
use std::time::Duration;

use chrono::Utc;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

use crate::StoreError;

/// A cached provider response together with the hash of the raw response body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedResponse {
    pub response_json: String,
    pub raw_hash: Option<String>,
}

/// SQLite-backed cache for normalized provider requests.
#[derive(Debug, Clone)]
pub struct SqliteCache {
    pool: SqlitePool,
}

impl SqliteCache {
    /// Open (or create) the cache database.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS query_cache (
                key           TEXT PRIMARY KEY NOT NULL,
                provider      TEXT NOT NULL,
                request_json  TEXT NOT NULL,
                response_json TEXT NOT NULL,
                raw_hash      TEXT,
                created_at    INTEGER NOT NULL,
                expires_at    INTEGER NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_query_cache_expires ON query_cache(expires_at);",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    /// Look up a live cache entry. Expired entries are ignored.
    pub async fn get(&self, key: &str) -> Result<Option<CachedResponse>, StoreError> {
        let now = Utc::now().timestamp();
        let row = sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT response_json, raw_hash FROM query_cache WHERE key = ? AND expires_at > ?",
        )
        .bind(key)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(response_json, raw_hash)| CachedResponse {
            response_json,
            raw_hash,
        }))
    }

    /// Insert or replace a cache entry.
    #[allow(clippy::too_many_arguments)]
    pub async fn put(
        &self,
        key: &str,
        provider: &str,
        request_json: &str,
        response_json: &str,
        raw_hash: Option<&str>,
        ttl_seconds: u64,
    ) -> Result<(), StoreError> {
        let now = Utc::now().timestamp();
        let expires_at = now.saturating_add(ttl_seconds.min(i64::MAX as u64) as i64);

        sqlx::query(
            r#"
            INSERT INTO query_cache
                (key, provider, request_json, response_json, raw_hash, created_at, expires_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(key) DO UPDATE SET
                provider = excluded.provider,
                request_json = excluded.request_json,
                response_json = excluded.response_json,
                raw_hash = excluded.raw_hash,
                created_at = excluded.created_at,
                expires_at = excluded.expires_at
            "#,
        )
        .bind(key)
        .bind(provider)
        .bind(request_json)
        .bind(response_json)
        .bind(raw_hash)
        .bind(now)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Remove expired entries. Returns the number deleted.
    pub async fn purge_expired(&self) -> Result<u64, StoreError> {
        let now = Utc::now().timestamp();
        let result = sqlx::query("DELETE FROM query_cache WHERE expires_at <= ?")
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cache_roundtrip_and_expiry() {
        let dir = tempfile::tempdir().unwrap();
        let cache = SqliteCache::open(dir.path().join("cache.sqlite"))
            .await
            .unwrap();

        assert!(cache.get("missing").await.unwrap().is_none());

        cache
            .put("k", "tavily", "{}", r#"{"ok":true}"#, Some("abc"), 60)
            .await
            .unwrap();

        let hit = cache.get("k").await.unwrap().unwrap();
        assert_eq!(hit.response_json, r#"{"ok":true}"#);
        assert_eq!(hit.raw_hash.as_deref(), Some("abc"));

        cache
            .put("short", "tavily", "{}", "{}", None, 0)
            .await
            .unwrap();
        assert!(cache.get("short").await.unwrap().is_none());
    }
}
