use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use research_core::{Job, JobEvent, JobId, JobStatus, ResearchRequest};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use tokio::sync::Mutex;

use crate::StoreError;

const ACTIVE_STATUSES: &str =
    "'planning', 'awaiting-review', 'running', 'synthesizing', 'auditing'";

/// Result of enqueueing a request. `created` is false when an idempotency key
/// matched an existing job.
#[derive(Debug, Clone)]
pub struct EnqueueOutcome {
    pub job: Job,
    pub created: bool,
}

/// A job whose lease expired. Cancellation requests are honoured even when the
/// worker that held the lease never came back.
#[derive(Debug, Clone)]
pub struct ReapedJob {
    pub id: JobId,
    pub status: JobStatus,
}

/// Durable job queue backed by SQLite.
///
/// Workers claim a queued job with a lease, refresh that lease with heartbeats,
/// and advance the job through the state machine in `research_core`. Expired
/// leases are reaped into `Interrupted` so a restarted worker can resume them.
#[derive(Debug, Clone)]
pub struct JobStore {
    pool: SqlitePool,
    /// SQLite permits one writer at a time, and a deferred transaction that
    /// upgrades from read to write can fail with `SQLITE_BUSY` instead of
    /// waiting. Serializing writes inside this process avoids that race
    /// between the worker and its heartbeat task.
    write_lock: Arc<Mutex<()>>,
}

impl JobStore {
    /// Open (or create) the job database.
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

    /// Add a job to the queue.
    ///
    /// When `idempotency_key` is present and already known, the existing job is
    /// returned unchanged instead of creating a duplicate.
    pub async fn enqueue(
        &self,
        request: ResearchRequest,
        output_dir: impl Into<PathBuf>,
        idempotency_key: Option<&str>,
    ) -> Result<EnqueueOutcome, StoreError> {
        request
            .validate()
            .map_err(|err| StoreError::InvalidRequest(err.to_string()))?;

        let _guard = self.write_lock.lock().await;
        let idempotency_key = idempotency_key.map(str::trim).filter(|key| !key.is_empty());
        let id = JobId::new();
        let now = Utc::now();
        let output_dir = output_dir.into();
        let output_dir = if output_dir.is_absolute() {
            output_dir
        } else {
            std::env::current_dir()?.join(output_dir)
        };
        let mut request = request;
        request.output_dir = Some(output_dir.clone());
        let request_json =
            serde_json::to_string(&request).map_err(|err| StoreError::Data(err.to_string()))?;

        let inserted = sqlx::query(
            r#"
            INSERT INTO jobs
                (id, idempotency_key, request_json, status, output_dir,
                 created_at, updated_at, attempts, cancel_requested)
            VALUES (?, ?, ?, ?, ?, ?, ?, 0, 0)
            ON CONFLICT(idempotency_key) DO NOTHING
            "#,
        )
        .bind(id.to_string())
        .bind(idempotency_key)
        .bind(request_json)
        .bind(JobStatus::Queued.as_str())
        .bind(output_dir.to_string_lossy().as_ref())
        .bind(millis(now))
        .bind(millis(now))
        .execute(&self.pool)
        .await?;

        if inserted.rows_affected() == 1 {
            self.insert_event(id, now, JobStatus::Queued, Some("enqueued"))
                .await?;
            return Ok(EnqueueOutcome {
                job: self.require(id).await?,
                created: true,
            });
        }

        let key = idempotency_key.ok_or_else(|| {
            StoreError::Data("insert conflicted without an idempotency key".to_owned())
        })?;
        let job = self
            .find_by_idempotency_key(key)
            .await?
            .ok_or_else(|| StoreError::Data(format!("idempotency key {key} vanished")))?;

        Ok(EnqueueOutcome {
            job,
            created: false,
        })
    }

    pub async fn get(&self, id: JobId) -> Result<Option<Job>, StoreError> {
        let row = sqlx::query("SELECT * FROM jobs WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        row.as_ref().map(job_from_row).transpose()
    }

    pub async fn require(&self, id: JobId) -> Result<Job, StoreError> {
        self.get(id).await?.ok_or(StoreError::JobNotFound(id))
    }

    async fn find_by_idempotency_key(&self, key: &str) -> Result<Option<Job>, StoreError> {
        let row = sqlx::query("SELECT * FROM jobs WHERE idempotency_key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;

        row.as_ref().map(job_from_row).transpose()
    }

    /// List jobs, newest first. `limit` is clamped to 1..=500.
    pub async fn list(
        &self,
        status: Option<JobStatus>,
        limit: u32,
    ) -> Result<Vec<Job>, StoreError> {
        let limit = i64::from(limit.clamp(1, 500));
        let rows = match status {
            Some(status) => {
                sqlx::query("SELECT * FROM jobs WHERE status = ? ORDER BY created_at DESC LIMIT ?")
                    .bind(status.as_str())
                    .bind(limit)
                    .fetch_all(&self.pool)
                    .await?
            }
            None => {
                sqlx::query("SELECT * FROM jobs ORDER BY created_at DESC LIMIT ?")
                    .bind(limit)
                    .fetch_all(&self.pool)
                    .await?
            }
        };

        rows.iter().map(job_from_row).collect()
    }

    /// Claim the oldest queued job and move it to `Planning` under a lease.
    ///
    /// Returns `None` when the queue is empty. The update re-checks the status,
    /// so two workers racing for the same row cannot both succeed.
    pub async fn claim(&self, owner: &str, lease: Duration) -> Result<Option<Job>, StoreError> {
        let _guard = self.write_lock.lock().await;
        let now = Utc::now();
        let expires = lease_deadline(now, lease);

        for _ in 0..8 {
            let mut tx = self.pool.begin().await?;
            let row = sqlx::query(
                "SELECT id FROM jobs WHERE status = ? AND cancel_requested = 0 \
                 ORDER BY created_at ASC LIMIT 1",
            )
            .bind(JobStatus::Queued.as_str())
            .fetch_optional(&mut *tx)
            .await?;

            let Some(row) = row else {
                tx.rollback().await?;
                return Ok(None);
            };
            let id_text: String = row.try_get("id")?;

            let updated = sqlx::query(
                r#"
                UPDATE jobs
                SET status = ?, lease_owner = ?, lease_expires_at = ?,
                    updated_at = ?, attempts = attempts + 1
                WHERE id = ? AND status = ?
                "#,
            )
            .bind(JobStatus::Planning.as_str())
            .bind(owner)
            .bind(millis(expires))
            .bind(millis(now))
            .bind(&id_text)
            .bind(JobStatus::Queued.as_str())
            .execute(&mut *tx)
            .await?;

            if updated.rows_affected() == 1 {
                insert_event_tx(&mut tx, &id_text, now, JobStatus::Planning, Some(owner)).await?;
                tx.commit().await?;
                return Ok(Some(self.require(parse_job_id(&id_text)?).await?));
            }

            tx.rollback().await?;
        }

        Ok(None)
    }

    /// Claim a specific queued job. Used by `research run` so it does not
    /// process unrelated jobs that are ahead in the queue.
    pub async fn claim_job(
        &self,
        id: JobId,
        owner: &str,
        lease: Duration,
    ) -> Result<Option<Job>, StoreError> {
        let _guard = self.write_lock.lock().await;
        let now = Utc::now();
        let expires = lease_deadline(now, lease);

        let updated = sqlx::query(
            r#"
            UPDATE jobs
            SET status = ?, lease_owner = ?, lease_expires_at = ?,
                updated_at = ?, attempts = attempts + 1
            WHERE id = ? AND status = ? AND cancel_requested = 0
            "#,
        )
        .bind(JobStatus::Planning.as_str())
        .bind(owner)
        .bind(millis(expires))
        .bind(millis(now))
        .bind(id.to_string())
        .bind(JobStatus::Queued.as_str())
        .execute(&self.pool)
        .await?;

        if updated.rows_affected() != 1 {
            return Ok(None);
        }

        self.insert_event(id, now, JobStatus::Planning, Some(owner))
            .await?;
        Ok(Some(self.require(id).await?))
    }

    /// Refresh a lease. Fails when the caller no longer owns the job, which is
    /// how a worker learns that its lease was reaped or taken over.
    pub async fn heartbeat(
        &self,
        id: JobId,
        owner: &str,
        lease: Duration,
    ) -> Result<Job, StoreError> {
        let _guard = self.write_lock.lock().await;
        let now = Utc::now();
        let expires = lease_deadline(now, lease);
        let updated = sqlx::query(&format!(
            "UPDATE jobs SET lease_expires_at = ?, updated_at = ? \
             WHERE id = ? AND lease_owner = ? AND status IN ({ACTIVE_STATUSES})"
        ))
        .bind(millis(expires))
        .bind(millis(now))
        .bind(id.to_string())
        .bind(owner)
        .execute(&self.pool)
        .await?;

        if updated.rows_affected() != 1 {
            return Err(StoreError::LeaseNotHeld {
                id,
                owner: owner.to_owned(),
            });
        }

        self.require(id).await
    }

    /// Advance a job through the state machine.
    pub async fn transition(&self, id: JobId, next: JobStatus) -> Result<Job, StoreError> {
        self.set_status(id, next, None, None, false, false).await
    }

    /// Mark a job failed and preserve the reason.
    pub async fn fail(&self, id: JobId, message: &str) -> Result<Job, StoreError> {
        self.set_status(
            id,
            JobStatus::Failed,
            Some(message),
            Some(message),
            false,
            false,
        )
        .await
    }

    /// Ask a job to stop.
    ///
    /// A queued job is cancelled immediately. An active job gets a cooperative
    /// flag; the worker sees it on its next heartbeat and transitions the job to
    /// `Cancelled`. Cancelling an already cancelled job is a no-op.
    pub async fn cancel(&self, id: JobId) -> Result<Job, StoreError> {
        let _guard = self.write_lock.lock().await;
        let mut tx = self.pool.begin().await?;
        let Some(row) = fetch_row_tx(&mut tx, id).await? else {
            tx.rollback().await?;
            return Err(StoreError::JobNotFound(id));
        };
        let job = job_from_row(&row)?;

        if job.status == JobStatus::Cancelled {
            tx.rollback().await?;
            return Ok(job);
        }
        if job.status == JobStatus::Completed {
            tx.rollback().await?;
            return Err(StoreError::InvalidTransition {
                from: job.status,
                to: JobStatus::Cancelled,
            });
        }

        let now = Utc::now();
        let next = if job.status == JobStatus::Queued {
            JobStatus::Cancelled
        } else {
            job.status
        };
        let clear_lease = job.status == JobStatus::Queued;

        sqlx::query(
            r#"
            UPDATE jobs
            SET status = ?, cancel_requested = 1, updated_at = ?,
                lease_owner = CASE WHEN ? THEN NULL ELSE lease_owner END,
                lease_expires_at = CASE WHEN ? THEN NULL ELSE lease_expires_at END
            WHERE id = ?
            "#,
        )
        .bind(next.as_str())
        .bind(millis(now))
        .bind(clear_lease)
        .bind(clear_lease)
        .bind(id.to_string())
        .execute(&mut *tx)
        .await?;

        insert_event_tx(
            &mut tx,
            &id.to_string(),
            now,
            next,
            Some("cancellation requested"),
        )
        .await?;
        tx.commit().await?;

        self.require(id).await
    }

    /// Approve a plan awaiting review and put the job back on the queue so a
    /// worker can collect evidence.
    pub async fn approve_plan(&self, id: JobId) -> Result<Job, StoreError> {
        self.set_status(
            id,
            JobStatus::Queued,
            Some("plan approved"),
            None,
            false,
            false,
        )
        .await
    }

    /// Requeue an interrupted or failed job.
    pub async fn resume(&self, id: JobId) -> Result<Job, StoreError> {
        let job = self.require(id).await?;
        if !job.status.is_resumable() {
            return Err(StoreError::NotResumable {
                id,
                status: job.status,
            });
        }

        self.set_status(id, JobStatus::Queued, Some("resumed"), None, true, true)
            .await
    }

    /// Move jobs whose leases expired out of active states.
    ///
    /// A job whose worker is gone becomes `Interrupted` so it can resume. If
    /// cancellation was requested before the worker disappeared, it becomes
    /// `Cancelled` instead; the request is not lost with the lease.
    ///
    /// Called on daemon startup and periodically by the service.
    pub async fn reap_expired_leases(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<ReapedJob>, StoreError> {
        let _guard = self.write_lock.lock().await;
        let mut tx = self.pool.begin().await?;
        let rows = sqlx::query(&format!(
            "SELECT id, cancel_requested FROM jobs \
             WHERE lease_expires_at IS NOT NULL AND lease_expires_at <= ? \
             AND status IN ({ACTIVE_STATUSES})"
        ))
        .bind(millis(now))
        .fetch_all(&mut *tx)
        .await?;

        let mut reaped = Vec::with_capacity(rows.len());
        for row in rows {
            let id_text: String = row.try_get("id")?;
            let id = parse_job_id(&id_text)?;
            let cancel_requested: i64 = row.try_get("cancel_requested")?;
            let (status, detail) = if cancel_requested != 0 {
                (JobStatus::Cancelled, "cancelled after lease expiry")
            } else {
                (JobStatus::Interrupted, "lease expired")
            };

            sqlx::query(
                "UPDATE jobs SET status = ?, lease_owner = NULL, \
                 lease_expires_at = NULL, updated_at = ? WHERE id = ?",
            )
            .bind(status.as_str())
            .bind(millis(now))
            .bind(&id_text)
            .execute(&mut *tx)
            .await?;

            insert_event_tx(&mut tx, &id_text, now, status, Some(detail)).await?;
            reaped.push(ReapedJob { id, status });
        }

        tx.commit().await?;
        Ok(reaped)
    }

    /// Read a job's full transition history.
    pub async fn events(&self, id: JobId) -> Result<Vec<JobEvent>, StoreError> {
        let rows = sqlx::query(
            "SELECT at, status, detail FROM job_events WHERE job_id = ? ORDER BY id ASC",
        )
        .bind(id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter()
            .map(|row| {
                let at: i64 = row.try_get("at")?;
                let status: String = row.try_get("status")?;
                let detail: Option<String> = row.try_get("detail")?;
                Ok(JobEvent {
                    at: datetime(at)?,
                    status: status
                        .parse()
                        .map_err(|err: research_core::ParseJobStatusError| {
                            StoreError::Data(err.to_string())
                        })?,
                    detail,
                })
            })
            .collect()
    }

    async fn set_status(
        &self,
        id: JobId,
        next: JobStatus,
        detail: Option<&str>,
        last_error: Option<&str>,
        clear_error: bool,
        reset_cancel: bool,
    ) -> Result<Job, StoreError> {
        let _guard = self.write_lock.lock().await;
        let mut tx = self.pool.begin().await?;
        let Some(row) = fetch_row_tx(&mut tx, id).await? else {
            tx.rollback().await?;
            return Err(StoreError::JobNotFound(id));
        };
        let job = job_from_row(&row)?;

        if !job.status.can_transition_to(next) {
            tx.rollback().await?;
            return Err(StoreError::InvalidTransition {
                from: job.status,
                to: next,
            });
        }

        let now = Utc::now();
        let clear_lease =
            next.is_terminal() || matches!(next, JobStatus::Queued | JobStatus::Interrupted);

        sqlx::query(
            r#"
            UPDATE jobs
            SET status = ?, updated_at = ?,
                lease_owner = CASE WHEN ? THEN NULL ELSE lease_owner END,
                lease_expires_at = CASE WHEN ? THEN NULL ELSE lease_expires_at END,
                last_error = CASE WHEN ? THEN NULL ELSE COALESCE(?, last_error) END,
                cancel_requested = CASE WHEN ? THEN 0 ELSE cancel_requested END
            WHERE id = ? AND status = ?
            "#,
        )
        .bind(next.as_str())
        .bind(millis(now))
        .bind(clear_lease)
        .bind(clear_lease)
        .bind(clear_error)
        .bind(last_error)
        .bind(reset_cancel)
        .bind(id.to_string())
        .bind(job.status.as_str())
        .execute(&mut *tx)
        .await?;

        insert_event_tx(&mut tx, &id.to_string(), now, next, detail).await?;
        tx.commit().await?;

        self.require(id).await
    }

    async fn insert_event(
        &self,
        id: JobId,
        at: DateTime<Utc>,
        status: JobStatus,
        detail: Option<&str>,
    ) -> Result<(), StoreError> {
        sqlx::query("INSERT INTO job_events (job_id, at, status, detail) VALUES (?, ?, ?, ?)")
            .bind(id.to_string())
            .bind(millis(at))
            .bind(status.as_str())
            .bind(detail)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

async fn install_schema(pool: &SqlitePool) -> Result<(), StoreError> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS jobs (
            id               TEXT PRIMARY KEY NOT NULL,
            idempotency_key  TEXT UNIQUE,
            request_json     TEXT NOT NULL,
            status           TEXT NOT NULL,
            output_dir       TEXT NOT NULL,
            created_at       INTEGER NOT NULL,
            updated_at       INTEGER NOT NULL,
            lease_owner      TEXT,
            lease_expires_at INTEGER,
            attempts         INTEGER NOT NULL DEFAULT 0,
            last_error       TEXT,
            cancel_requested INTEGER NOT NULL DEFAULT 0
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_jobs_status_created ON jobs(status, created_at);")
        .execute(pool)
        .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS job_events (
            id      INTEGER PRIMARY KEY AUTOINCREMENT,
            job_id  TEXT NOT NULL REFERENCES jobs(id),
            at      INTEGER NOT NULL,
            status  TEXT NOT NULL,
            detail  TEXT
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_job_events_job ON job_events(job_id, id);")
        .execute(pool)
        .await?;

    Ok(())
}

async fn fetch_row_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: JobId,
) -> Result<Option<SqliteRow>, StoreError> {
    Ok(sqlx::query("SELECT * FROM jobs WHERE id = ?")
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await?)
}

async fn insert_event_tx(
    tx: &mut Transaction<'_, Sqlite>,
    job_id: &str,
    at: DateTime<Utc>,
    status: JobStatus,
    detail: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query("INSERT INTO job_events (job_id, at, status, detail) VALUES (?, ?, ?, ?)")
        .bind(job_id)
        .bind(millis(at))
        .bind(status.as_str())
        .bind(detail)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn job_from_row(row: &SqliteRow) -> Result<Job, StoreError> {
    let id_text: String = row.try_get("id")?;
    let request_json: String = row.try_get("request_json")?;

    let request: ResearchRequest =
        serde_json::from_str(&request_json).map_err(|err| StoreError::Data(err.to_string()))?;
    let status_text: String = row.try_get("status")?;
    let status: JobStatus = status_text
        .parse()
        .map_err(|err: research_core::ParseJobStatusError| StoreError::Data(err.to_string()))?;
    let output_dir: String = row.try_get("output_dir")?;
    let attempts: i64 = row.try_get("attempts")?;
    let cancel_requested: i64 = row.try_get("cancel_requested")?;

    Ok(Job {
        id: parse_job_id(&id_text)?,
        request,
        status,
        output_dir: PathBuf::from(output_dir),
        created_at: datetime(row.try_get("created_at")?)?,
        updated_at: datetime(row.try_get("updated_at")?)?,
        lease_owner: row.try_get("lease_owner")?,
        lease_expires_at: row
            .try_get::<Option<i64>, _>("lease_expires_at")?
            .map(datetime)
            .transpose()?,
        attempts: u32::try_from(attempts)
            .map_err(|_| StoreError::Data(format!("invalid attempts: {attempts}")))?,
        last_error: row.try_get("last_error")?,
        idempotency_key: row.try_get("idempotency_key")?,
        cancel_requested: cancel_requested != 0,
    })
}

fn parse_job_id(text: &str) -> Result<JobId, StoreError> {
    JobId::from_str(text).map_err(|err| StoreError::Data(err.to_string()))
}

fn millis(at: DateTime<Utc>) -> i64 {
    at.timestamp_millis()
}

fn datetime(ms: i64) -> Result<DateTime<Utc>, StoreError> {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .ok_or_else(|| StoreError::Data(format!("invalid timestamp: {ms}")))
}

fn lease_deadline(now: DateTime<Utc>, lease: Duration) -> DateTime<Utc> {
    let millis = i64::try_from(lease.as_millis()).unwrap_or(i64::MAX);
    now + chrono::Duration::milliseconds(millis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use research_core::{Depth, ResearchRequest};

    async fn store() -> (tempfile::TempDir, JobStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = JobStore::open(dir.path().join("jobs.sqlite"))
            .await
            .unwrap();
        (dir, store)
    }

    fn request(question: &str) -> ResearchRequest {
        let mut request = ResearchRequest::new(question);
        request.depth = Depth::Quick;
        request
    }

    #[tokio::test]
    async fn enqueue_is_idempotent() {
        let (_dir, store) = store().await;

        let first = store
            .enqueue(
                request("first question"),
                "research/first",
                Some("same-key"),
            )
            .await
            .unwrap();
        let second = store
            .enqueue(
                request("different question"),
                "research/second",
                Some("same-key"),
            )
            .await
            .unwrap();

        assert!(first.created);
        assert!(!second.created);
        assert_eq!(first.job.id, second.job.id);
        assert_eq!(second.job.request.question, "first question");
        assert_eq!(store.list(None, 10).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn enqueue_makes_output_dirs_absolute() {
        let (_dir, store) = store().await;
        let job = store
            .enqueue(request("relative"), "relative-output", None)
            .await
            .unwrap()
            .job;

        assert!(job.output_dir.is_absolute());
        assert_eq!(job.request.output_dir.as_ref(), Some(&job.output_dir));
    }

    #[tokio::test]
    async fn queued_jobs_are_claimed_in_order_under_a_lease() {
        let (_dir, store) = store().await;
        let first = store
            .enqueue(request("first"), "research/first", None)
            .await
            .unwrap()
            .job;
        let second = store
            .enqueue(request("second"), "research/second", None)
            .await
            .unwrap()
            .job;

        let claimed = store
            .claim("worker-a", Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claimed.id, first.id);
        assert_eq!(claimed.status, JobStatus::Planning);
        assert_eq!(claimed.lease_owner.as_deref(), Some("worker-a"));
        assert_eq!(claimed.attempts, 1);

        let claimed = store
            .claim("worker-b", Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claimed.id, second.id);
        assert!(
            store
                .claim("worker-c", Duration::from_secs(60))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn heartbeat_refreshes_only_the_owner() {
        let (_dir, store) = store().await;
        let job = store
            .enqueue(request("heartbeat"), "research/heartbeat", None)
            .await
            .unwrap()
            .job;
        let claimed = store
            .claim("worker-a", Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();

        let refreshed = store
            .heartbeat(claimed.id, "worker-a", Duration::from_secs(120))
            .await
            .unwrap();
        assert!(refreshed.lease_expires_at.unwrap() > claimed.lease_expires_at.unwrap());

        assert!(matches!(
            store
                .heartbeat(job.id, "worker-b", Duration::from_secs(60))
                .await,
            Err(StoreError::LeaseNotHeld { .. })
        ));
    }

    #[tokio::test]
    async fn expired_leases_are_interrupted_and_resumed() {
        let (_dir, store) = store().await;
        let job = store
            .enqueue(request("recover me"), "research/recover", None)
            .await
            .unwrap()
            .job;

        store
            .claim("worker-a", Duration::from_secs(0))
            .await
            .unwrap()
            .unwrap();

        let reaped = store.reap_expired_leases(Utc::now()).await.unwrap();
        assert_eq!(reaped.len(), 1);
        assert_eq!(reaped[0].id, job.id);
        assert_eq!(reaped[0].status, JobStatus::Interrupted);

        let interrupted = store.require(job.id).await.unwrap();
        assert_eq!(interrupted.status, JobStatus::Interrupted);
        assert!(interrupted.lease_owner.is_none());

        let resumed = store.resume(job.id).await.unwrap();
        assert_eq!(resumed.status, JobStatus::Queued);
        assert_eq!(resumed.attempts, 1);

        let claimed = store
            .claim("worker-b", Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claimed.id, job.id);
        assert_eq!(claimed.attempts, 2);
    }

    #[tokio::test]
    async fn cancelling_queued_jobs_is_immediate() {
        let (_dir, store) = store().await;
        let job = store
            .enqueue(request("cancel me"), "research/cancel", None)
            .await
            .unwrap()
            .job;

        let cancelled = store.cancel(job.id).await.unwrap();
        assert_eq!(cancelled.status, JobStatus::Cancelled);
        assert!(cancelled.is_cancellation_requested());
        assert!(
            store
                .claim("worker", Duration::from_secs(60))
                .await
                .unwrap()
                .is_none()
        );

        // Cancelling twice is harmless.
        assert_eq!(
            store.cancel(job.id).await.unwrap().status,
            JobStatus::Cancelled
        );
    }

    #[tokio::test]
    async fn cancelling_active_jobs_is_cooperative() {
        let (_dir, store) = store().await;
        let job = store
            .enqueue(request("cooperative cancel"), "research/coop", None)
            .await
            .unwrap()
            .job;
        store
            .claim("worker-a", Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();

        let requested = store.cancel(job.id).await.unwrap();
        assert_eq!(requested.status, JobStatus::Planning);
        assert!(requested.is_cancellation_requested());

        let cancelled = store
            .transition(job.id, JobStatus::Cancelled)
            .await
            .unwrap();
        assert_eq!(cancelled.status, JobStatus::Cancelled);
    }

    #[tokio::test]
    async fn invalid_transitions_are_rejected() {
        let (_dir, store) = store().await;
        let job = store
            .enqueue(request("illegal"), "research/illegal", None)
            .await
            .unwrap()
            .job;

        assert!(matches!(
            store.transition(job.id, JobStatus::Completed).await,
            Err(StoreError::InvalidTransition { .. })
        ));
    }

    #[tokio::test]
    async fn cancellation_survives_a_dead_worker() {
        let (_dir, store) = store().await;
        let job = store
            .enqueue(request("dead worker"), "research/dead", None)
            .await
            .unwrap()
            .job;
        store
            .claim("worker-a", Duration::from_secs(0))
            .await
            .unwrap()
            .unwrap();

        let requested = store.cancel(job.id).await.unwrap();
        assert_eq!(requested.status, JobStatus::Planning);
        assert!(requested.cancel_requested);

        let reaped = store.reap_expired_leases(Utc::now()).await.unwrap();
        assert_eq!(reaped.len(), 1);
        assert_eq!(reaped[0].status, JobStatus::Cancelled);

        let cancelled = store.require(job.id).await.unwrap();
        assert_eq!(cancelled.status, JobStatus::Cancelled);
        assert!(cancelled.lease_owner.is_none());
    }

    #[tokio::test]
    async fn failed_jobs_can_be_resumed() {
        let (_dir, store) = store().await;
        let job = store
            .enqueue(request("fail then retry"), "research/retry", None)
            .await
            .unwrap()
            .job;
        store
            .claim("worker-a", Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();

        let failed = store.fail(job.id, "provider exploded").await.unwrap();
        assert_eq!(failed.status, JobStatus::Failed);
        assert_eq!(failed.last_error.as_deref(), Some("provider exploded"));

        let resumed = store.resume(job.id).await.unwrap();
        assert_eq!(resumed.status, JobStatus::Queued);
        assert!(resumed.last_error.is_none());
    }

    #[tokio::test]
    async fn events_record_the_history() {
        let (_dir, store) = store().await;
        let job = store
            .enqueue(request("history"), "research/history", None)
            .await
            .unwrap()
            .job;
        store
            .claim("worker-a", Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();
        store.transition(job.id, JobStatus::Running).await.unwrap();

        let events = store.events(job.id).await.unwrap();
        let statuses: Vec<_> = events.iter().map(|event| event.status).collect();
        assert_eq!(
            statuses,
            vec![JobStatus::Queued, JobStatus::Planning, JobStatus::Running]
        );
    }

    #[tokio::test]
    async fn empty_questions_are_rejected_before_insert() {
        let (_dir, store) = store().await;
        assert!(matches!(
            store.enqueue(request("   "), "research/empty", None).await,
            Err(StoreError::InvalidRequest(_))
        ));
    }
}
