use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use research_core::JobId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use tokio::sync::Mutex;

use crate::StoreError;

/// One agent run's metadata. The event stream is append-only and keyed by the
/// same job ID, so a resumed job keeps one transcript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentRun {
    pub job_id: JobId,
    pub engine: String,
    pub provider: String,
    pub model: String,
    pub prompt_version: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: String,
    pub stop_reason: Option<String>,
    pub usage: Value,
}

/// One recorded model turn, tool call, budget decision, or lifecycle event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentEvent {
    pub id: i64,
    pub job_id: JobId,
    pub seq: i64,
    pub at: DateTime<Utc>,
    pub kind: String,
    pub payload: Value,
}

/// SQLite transcript store for the agent loop.
///
/// It shares the job database file. The agent transcript is deliberately
/// separate from `job_events`: job events are lifecycle transitions, while
/// agent events include full prompts, model responses, tool arguments, and tool
/// results for replay and debugging.
#[derive(Debug, Clone)]
pub struct AgentStore {
    pool: SqlitePool,
    write_lock: Arc<Mutex<()>>,
}

impl AgentStore {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5))
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;

        install_schema(&pool).await?;
        Ok(Self {
            pool,
            write_lock: Arc::new(Mutex::new(())),
        })
    }

    /// Create the run row if this job has not started an agent run before.
    pub async fn start_run(&self, run: &AgentRun) -> Result<(), StoreError> {
        let usage =
            serde_json::to_string(&run.usage).map_err(|err| StoreError::Data(err.to_string()))?;
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agent_runs
                (job_id, engine, provider, model, prompt_version, started_at,
                 finished_at, status, stop_reason, usage_json)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(run.job_id.to_string())
        .bind(&run.engine)
        .bind(&run.provider)
        .bind(&run.model)
        .bind(&run.prompt_version)
        .bind(millis(run.started_at))
        .bind(run.finished_at.map(millis))
        .bind(&run.status)
        .bind(&run.stop_reason)
        .bind(usage)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Append one event and return its per-job sequence number.
    pub async fn append(
        &self,
        job_id: JobId,
        kind: impl Into<String>,
        payload: Value,
    ) -> Result<i64, StoreError> {
        let _guard = self.write_lock.lock().await;
        let seq: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM agent_events WHERE job_id = ?",
        )
        .bind(job_id.to_string())
        .fetch_one(&self.pool)
        .await?;
        let payload =
            serde_json::to_string(&payload).map_err(|err| StoreError::Data(err.to_string()))?;
        sqlx::query(
            "INSERT INTO agent_events (job_id, seq, at, kind, payload_json) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(job_id.to_string())
        .bind(seq)
        .bind(millis(Utc::now()))
        .bind(kind.into())
        .bind(payload)
        .execute(&self.pool)
        .await?;
        Ok(seq)
    }

    pub async fn finish_run(
        &self,
        job_id: JobId,
        status: &str,
        stop_reason: Option<&str>,
        usage: &Value,
    ) -> Result<(), StoreError> {
        let usage =
            serde_json::to_string(usage).map_err(|err| StoreError::Data(err.to_string()))?;
        sqlx::query(
            "UPDATE agent_runs SET finished_at = ?, status = ?, stop_reason = ?, \
             usage_json = ? WHERE job_id = ?",
        )
        .bind(millis(Utc::now()))
        .bind(status)
        .bind(stop_reason)
        .bind(usage)
        .bind(job_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn run(&self, job_id: JobId) -> Result<Option<AgentRun>, StoreError> {
        let row = sqlx::query("SELECT * FROM agent_runs WHERE job_id = ?")
            .bind(job_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(run_from_row).transpose()
    }

    pub async fn events(&self, job_id: JobId) -> Result<Vec<AgentEvent>, StoreError> {
        let rows = sqlx::query("SELECT * FROM agent_events WHERE job_id = ? ORDER BY seq ASC")
            .bind(job_id.to_string())
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(event_from_row).collect()
    }
}

async fn install_schema(pool: &SqlitePool) -> Result<(), StoreError> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS agent_runs (
            job_id          TEXT PRIMARY KEY NOT NULL,
            engine          TEXT NOT NULL,
            provider        TEXT NOT NULL,
            model           TEXT NOT NULL,
            prompt_version  TEXT NOT NULL,
            started_at      INTEGER NOT NULL,
            finished_at     INTEGER,
            status          TEXT NOT NULL,
            stop_reason     TEXT,
            usage_json      TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS agent_events (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            job_id        TEXT NOT NULL,
            seq           INTEGER NOT NULL,
            at            INTEGER NOT NULL,
            kind          TEXT NOT NULL,
            payload_json  TEXT NOT NULL,
            UNIQUE(job_id, seq)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_agent_events_job ON agent_events(job_id, seq);")
        .execute(pool)
        .await?;

    Ok(())
}

fn run_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<AgentRun, StoreError> {
    let job_id: String = row.try_get("job_id")?;
    let usage_json: String = row.try_get("usage_json")?;
    Ok(AgentRun {
        job_id: job_id
            .parse()
            .map_err(|err: uuid::Error| StoreError::Data(err.to_string()))?,
        engine: row.try_get("engine")?,
        provider: row.try_get("provider")?,
        model: row.try_get("model")?,
        prompt_version: row.try_get("prompt_version")?,
        started_at: datetime(row.try_get("started_at")?)?,
        finished_at: row
            .try_get::<Option<i64>, _>("finished_at")?
            .map(datetime)
            .transpose()?,
        status: row.try_get("status")?,
        stop_reason: row.try_get("stop_reason")?,
        usage: serde_json::from_str(&usage_json)
            .map_err(|err| StoreError::Data(err.to_string()))?,
    })
}

fn event_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<AgentEvent, StoreError> {
    let job_id: String = row.try_get("job_id")?;
    let payload_json: String = row.try_get("payload_json")?;
    Ok(AgentEvent {
        id: row.try_get("id")?,
        job_id: job_id
            .parse()
            .map_err(|err: uuid::Error| StoreError::Data(err.to_string()))?,
        seq: row.try_get("seq")?,
        at: datetime(row.try_get("at")?)?,
        kind: row.try_get("kind")?,
        payload: serde_json::from_str(&payload_json)
            .map_err(|err| StoreError::Data(err.to_string()))?,
    })
}

fn millis(at: DateTime<Utc>) -> i64 {
    at.timestamp_millis()
}

fn datetime(ms: i64) -> Result<DateTime<Utc>, StoreError> {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .ok_or_else(|| StoreError::Data(format!("invalid timestamp: {ms}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn runs_and_events_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentStore::open(dir.path().join("agent.sqlite"))
            .await
            .unwrap();
        let job_id = JobId::new();
        let run = AgentRun {
            job_id,
            engine: "agent-v1".to_owned(),
            provider: "opencode-go".to_owned(),
            model: "deepseek-v4.1-flash".to_owned(),
            prompt_version: "agent-v1".to_owned(),
            started_at: Utc::now(),
            finished_at: None,
            status: "running".to_owned(),
            stop_reason: None,
            usage: json!({}),
        };

        store.start_run(&run).await.unwrap();
        let seq = store
            .append(job_id, "action_request", json!({"prompt": "hello"}))
            .await
            .unwrap();
        assert_eq!(seq, 1);
        let seq = store
            .append(job_id, "action_response", json!({"content": "world"}))
            .await
            .unwrap();
        assert_eq!(seq, 2);

        let events = store.events(job_id).await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, "action_request");
        assert_eq!(events[1].payload["content"], "world");

        store
            .finish_run(
                job_id,
                "completed",
                Some("satisfied"),
                &json!({"total_tokens": 10}),
            )
            .await
            .unwrap();
        let finished = store.run(job_id).await.unwrap().unwrap();
        assert_eq!(finished.status, "completed");
        assert_eq!(finished.stop_reason.as_deref(), Some("satisfied"));
        assert!(finished.finished_at.is_some());
    }

    #[tokio::test]
    async fn a_second_start_does_not_reset_the_run() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentStore::open(dir.path().join("agent.sqlite"))
            .await
            .unwrap();
        let job_id = JobId::new();
        let first = AgentRun {
            job_id,
            engine: "agent-v1".to_owned(),
            provider: "opencode-go".to_owned(),
            model: "deepseek-v4.1-flash".to_owned(),
            prompt_version: "agent-v1".to_owned(),
            started_at: Utc::now(),
            finished_at: None,
            status: "running".to_owned(),
            stop_reason: None,
            usage: json!({}),
        };
        store.start_run(&first).await.unwrap();
        store
            .append(job_id, "run_start", json!({"attempt": 1}))
            .await
            .unwrap();

        let mut second = first.clone();
        second.started_at = Utc::now() + chrono::Duration::seconds(10);
        store.start_run(&second).await.unwrap();
        assert_eq!(
            store
                .run(job_id)
                .await
                .unwrap()
                .unwrap()
                .started_at
                .timestamp_millis(),
            first.started_at.timestamp_millis()
        );
    }
}
