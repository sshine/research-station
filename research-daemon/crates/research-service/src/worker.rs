use std::future::Future;
use std::time::Duration;

use chrono::Utc;
use research_agent::AgentRunner;
use research_core::{Config, Engine, Job, JobStatus};
use research_llm::AnyLlm;
use research_provider::SearchProvider;
use research_store::{AgentStore, JobStore};
use tokio::sync::watch;
use tokio::time::MissedTickBehavior;

use crate::ServiceError;
use crate::runner::{RunOutcome, Runner};

#[derive(Debug, Clone)]
pub struct WorkerOptions {
    /// Identity written on the lease. Use one per worker process.
    pub owner: String,
    /// How long a claim is valid without a heartbeat.
    pub lease: Duration,
    /// How long to wait before polling an empty queue again.
    pub poll_interval: Duration,
    /// How often to reap leases left by dead workers.
    pub reap_interval: Duration,
}

impl WorkerOptions {
    pub fn from_config(config: &Config, owner: impl Into<String>) -> Self {
        Self {
            owner: owner.into(),
            lease: Duration::from_secs(config.jobs.lease_seconds.max(1)),
            poll_interval: Duration::from_secs(config.jobs.poll_interval_seconds.max(1)),
            reap_interval: Duration::from_secs(config.jobs.reap_interval_seconds.max(1)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopSignal {
    Cancelled,
    LeaseLost,
}

pub struct Worker<P: SearchProvider> {
    store: JobStore,
    provider: P,
    llm: Option<AnyLlm>,
    transcripts: Option<AgentStore>,
    options: WorkerOptions,
}

impl<P: SearchProvider> Worker<P> {
    /// Deterministic worker without a model. Agent jobs fail clearly unless the
    /// worker is built with [`Worker::with_agent`].
    pub fn new(store: JobStore, provider: P, options: WorkerOptions) -> Self {
        Self {
            store,
            provider,
            llm: None,
            transcripts: None,
            options,
        }
    }

    /// Worker that can execute the default agent engine.
    pub fn with_agent(
        store: JobStore,
        provider: P,
        llm: AnyLlm,
        transcripts: AgentStore,
        options: WorkerOptions,
    ) -> Self {
        Self {
            store,
            provider,
            llm: Some(llm),
            transcripts: Some(transcripts),
            options,
        }
    }

    pub fn options(&self) -> &WorkerOptions {
        &self.options
    }

    /// Reap leases left behind by a previous process. Call once on startup.
    pub async fn reap_expired(&self) -> Result<usize, ServiceError> {
        let reaped = self.store.reap_expired_leases(Utc::now()).await?;
        for job in &reaped {
            tracing::warn!(job = %job.id, status = %job.status, "reaped expired lease");
        }
        Ok(reaped.len())
    }

    /// Claim and process at most one job. Returns false when the queue is empty.
    pub async fn run_once(&self) -> Result<bool, ServiceError> {
        let Some(job) = self
            .store
            .claim(&self.options.owner, self.options.lease)
            .await?
        else {
            return Ok(false);
        };

        tracing::info!(job = %job.id, question = %job.request.question, "claimed job");
        match self.process(job.clone()).await {
            Ok(outcome) => {
                tracing::info!(
                    job = %job.id,
                    status = %outcome.status,
                    sources = outcome.sources,
                    claims = outcome.claims,
                    "job finished"
                );
            }
            Err(ServiceError::Cancelled(_)) => {
                tracing::info!(job = %job.id, "job cancelled");
            }
            Err(err) => {
                tracing::warn!(job = %job.id, error = %err, "job failed");
            }
        }

        Ok(true)
    }

    /// Claim one specific job and run it. Returns `None` when the job is not
    /// queued (for example because a daemon already owns it).
    pub async fn run_job(
        &self,
        id: research_core::JobId,
    ) -> Result<Option<crate::RunOutcome>, ServiceError> {
        let Some(job) = self
            .store
            .claim_job(id, &self.options.owner, self.options.lease)
            .await?
        else {
            return Ok(None);
        };

        tracing::info!(job = %job.id, question = %job.request.question, "claimed specific job");
        let outcome = self.process(job.clone()).await?;
        tracing::info!(
            job = %job.id,
            status = %outcome.status,
            sources = outcome.sources,
            claims = outcome.claims,
            "job finished"
        );
        Ok(Some(outcome))
    }

    /// Drain the current queue. Useful for tests and `--once`.
    pub async fn run_until_idle(&self) -> Result<usize, ServiceError> {
        let mut processed = 0;
        while self.run_once().await? {
            processed += 1;
            if processed >= 10_000 {
                break;
            }
        }
        Ok(processed)
    }

    /// Poll the queue until `shutdown` resolves.
    pub async fn run_forever<F>(&self, shutdown: F) -> Result<(), ServiceError>
    where
        F: Future<Output = ()>,
    {
        tokio::pin!(shutdown);
        let mut reap = tokio::time::interval(self.options.reap_interval);
        reap.tick().await; // startup reaping is explicit

        loop {
            tokio::select! {
                _ = &mut shutdown => break,
                _ = reap.tick() => {
                    if let Err(err) = self.reap_expired().await {
                        tracing::warn!(error = %err, "lease reaper failed");
                    }
                }
                result = self.run_once() => {
                    match result {
                        Ok(true) => {}
                        Ok(false) => tokio::time::sleep(self.options.poll_interval).await,
                        Err(err) => {
                            tracing::error!(error = %err, "worker loop error");
                            tokio::time::sleep(self.options.poll_interval).await;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn process(&self, job: Job) -> Result<crate::RunOutcome, ServiceError> {
        let (signal_tx, mut signal_rx) = watch::channel(None::<StopSignal>);
        let (stop_tx, mut stop_rx) = watch::channel(false);
        let heartbeat_store = self.store.clone();
        let owner = self.options.owner.clone();
        let lease = self.options.lease;
        let id = job.id;

        let heartbeat = tokio::spawn(async move {
            let period = (lease / 3).max(Duration::from_secs(1));
            let mut ticker = tokio::time::interval(period);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

            loop {
                tokio::select! {
                    _ = stop_rx.changed() => return,
                    _ = ticker.tick() => {
                        match heartbeat_store.heartbeat(id, &owner, lease).await {
                            Ok(job) if job.cancel_requested => {
                                let _ = signal_tx.send(Some(StopSignal::Cancelled));
                                return;
                            }
                            Ok(_) => {}
                            Err(err) => {
                                tracing::warn!(job = %id, error = %err, "heartbeat failed");
                                let _ = signal_tx.send(Some(StopSignal::LeaseLost));
                                return;
                            }
                        }
                    }
                }
            }
        });

        let run = async {
            match job.request.engine {
                Engine::Agent => {
                    let llm = self.llm.as_ref().ok_or_else(|| {
                        ServiceError::Message(format!(
                            "job {} uses the agent engine but this worker has no model provider",
                            job.id
                        ))
                    })?;
                    let transcripts = self.transcripts.as_ref().ok_or_else(|| {
                        ServiceError::Message(format!(
                            "job {} uses the agent engine but this worker has no transcript store",
                            job.id
                        ))
                    })?;
                    let outcome = AgentRunner::new(&self.store, &self.provider, llm, transcripts)
                        .run(job.clone())
                        .await?;
                    Ok::<RunOutcome, ServiceError>(RunOutcome {
                        status: outcome.status,
                        report_path: outcome.report_path,
                        sources: outcome.sources,
                        claims: outcome.claims,
                        awaiting_review: outcome.awaiting_review,
                        stop_reason: outcome.stop_reason,
                        rounds: outcome.rounds,
                    })
                }
                Engine::Deterministic => {
                    Runner::new(&self.store, &self.provider)
                        .run(job.clone())
                        .await
                }
            }
        };

        let outcome = tokio::select! {
            result = tokio::time::timeout(job.request.budget.wall_time(), run) => {
                Some(result)
            }
            _ = signal_rx.changed() => None,
        };

        let _ = stop_tx.send(true);
        let _ = heartbeat.await;

        match outcome {
            Some(Ok(Ok(outcome))) => Ok(outcome),
            Some(Ok(Err(err))) => {
                self.fail(job.id, &err.to_string()).await;
                Err(err)
            }
            Some(Err(_elapsed)) => {
                self.fail(job.id, "wall-time budget exceeded").await;
                Err(ServiceError::Timeout(job.id))
            }
            None => {
                let signal = *signal_rx.borrow();
                match signal {
                    Some(StopSignal::Cancelled) => {
                        if let Err(err) = self.store.transition(job.id, JobStatus::Cancelled).await
                        {
                            tracing::warn!(job = %job.id, error = %err, "failed to record cancellation");
                        }
                        Err(ServiceError::Cancelled(job.id))
                    }
                    Some(StopSignal::LeaseLost) | None => Err(ServiceError::Message(format!(
                        "lost lease for job {}",
                        job.id
                    ))),
                }
            }
        }
    }

    async fn fail(&self, id: research_core::JobId, message: &str) {
        if let Err(err) = self.store.fail(id, message).await {
            tracing::warn!(job = %id, error = %err, "failed to record job failure");
        }
    }
}
