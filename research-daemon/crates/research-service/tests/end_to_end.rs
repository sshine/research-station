use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use research_core::{
    Depth, Engine, ExtractRequest, ExtractResponse, JobId, JobStatus, RedditMode, ResearchRequest,
    ReviewPolicy, SearchRequest, SearchResponse,
};
use research_llm::{AnyLlm, FakeLlm};
use research_pipeline::{Bundle, RunMetadata};
use research_provider::{
    AnyReddit, FakeProvider, FakeRedditProvider, ProviderError, ProviderResponse, SearchProvider,
};
use research_service::{Worker, WorkerOptions};
use research_store::JobStore;

fn worker_options() -> WorkerOptions {
    WorkerOptions {
        owner: "test-worker".to_owned(),
        lease: Duration::from_secs(30),
        poll_interval: Duration::from_millis(10),
        reap_interval: Duration::from_secs(60),
    }
}

fn request(question: &str, depth: Depth) -> ResearchRequest {
    let mut request = ResearchRequest::new(question);
    request.engine = Engine::Deterministic;
    request.depth = depth;
    request
}

#[tokio::test]
async fn agent_run_produces_a_transcript_and_bundle() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite"))
        .await
        .unwrap();
    let transcripts = research_store::AgentStore::open(dir.path().join("jobs.sqlite"))
        .await
        .unwrap();
    let bundle_dir = dir.path().join("research/2026-11-04-agent-run");
    let mut request = request("How do Tavily and Kagi compare?", Depth::Quick);
    request.engine = Engine::Agent;
    let job = store.enqueue(request, &bundle_dir, None).await.unwrap().job;

    let llm =
        research_service::build_llm_provider(&research_core::Config::default(), true).unwrap();
    let worker = Worker::with_agent(
        store.clone(),
        FakeProvider,
        llm,
        transcripts.clone(),
        worker_options(),
    );
    assert_eq!(worker.run_until_idle().await.unwrap(), 1);

    let job = store.require(job.id).await.unwrap();
    assert_eq!(job.status, JobStatus::Completed);

    let metadata: RunMetadata =
        serde_json::from_slice(&std::fs::read(bundle_dir.join("run.json")).unwrap()).unwrap();
    assert_eq!(metadata.engine, "agent-v1");
    assert_eq!(metadata.planner, "agent-v1");
    assert!(metadata.usage.llm_tokens > 0);
    assert!(metadata.agent_rounds > 0);
    assert_eq!(metadata.stop_reason.as_deref(), Some("satisfied"));

    let events = transcripts.events(job.id).await.unwrap();
    assert!(events.iter().any(|event| event.kind == "plan_response"));
    assert!(events.iter().any(|event| event.kind == "action_request"));
    assert!(events.iter().any(|event| event.kind == "tool_result"));
    assert!(
        events
            .iter()
            .any(|event| event.kind == "synthesis_response")
    );
    assert!(events.iter().any(|event| event.kind == "run_end"));
}

#[tokio::test]
async fn reddit_agent_adds_community_sources_and_records_the_trigger() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite"))
        .await
        .unwrap();
    let transcripts = research_store::AgentStore::open(dir.path().join("jobs.sqlite"))
        .await
        .unwrap();
    let bundle_dir = dir.path().join("research/2026-11-04-reddit-run");
    let mut request = request("What do people say about the fake topic?", Depth::Quick);
    request.engine = Engine::Agent;
    request.reddit = RedditMode::Auto;
    let job = store.enqueue(request, &bundle_dir, None).await.unwrap().job;

    let llm = AnyLlm::Fake(FakeLlm::agent_with_reddit());
    let worker = Worker::with_agent_reddit(
        store.clone(),
        FakeProvider,
        Some(AnyReddit::Fake(FakeRedditProvider)),
        llm,
        transcripts.clone(),
        worker_options(),
    );
    assert_eq!(worker.run_until_idle().await.unwrap(), 1);

    let job = store.require(job.id).await.unwrap();
    assert_eq!(job.status, JobStatus::Completed);

    let sources = std::fs::read_to_string(bundle_dir.join("sources.jsonl")).unwrap();
    assert!(sources.contains("reddit.com/r/faketest"));
    assert!(sources.contains("fake Reddit thread"));
    let first_source: serde_json::Value =
        serde_json::from_str(sources.lines().next().unwrap()).unwrap();
    assert_eq!(first_source["source_type"], "community");

    let events = transcripts.events(job.id).await.unwrap();
    assert!(events.iter().any(|event| event.kind == "reddit_enabled"));
    assert!(events.iter().any(|event| {
        event.kind == "tool_result"
            && event.payload.get("tool").and_then(|value| value.as_str()) == Some("reddit_search")
    }));
}

#[tokio::test]
async fn reddit_action_without_a_provider_falls_back_to_web_search() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite"))
        .await
        .unwrap();
    let transcripts = research_store::AgentStore::open(dir.path().join("jobs.sqlite"))
        .await
        .unwrap();
    let bundle_dir = dir.path().join("research/2026-11-04-reddit-fallback-run");
    let mut request = request("What do people say about the fake topic?", Depth::Quick);
    request.engine = Engine::Agent;
    request.reddit = RedditMode::Auto;
    let job = store.enqueue(request, &bundle_dir, None).await.unwrap().job;

    // The model asks for Reddit, but the worker has no Reddit provider.
    let llm = AnyLlm::Fake(FakeLlm::agent_with_reddit());
    let worker = Worker::with_agent(
        store.clone(),
        FakeProvider,
        llm,
        transcripts.clone(),
        worker_options(),
    );
    assert_eq!(worker.run_until_idle().await.unwrap(), 1);

    let job = store.require(job.id).await.unwrap();
    assert_eq!(job.status, JobStatus::Completed);

    let sources = std::fs::read_to_string(bundle_dir.join("sources.jsonl")).unwrap();
    assert!(!sources.contains("reddit.com/r/faketest"));
    assert!(sources.contains("example1.test"));

    let events = transcripts.events(job.id).await.unwrap();
    assert!(events.iter().any(|event| {
        event.kind == "tool_result"
            && event.payload.get("error").and_then(|value| value.as_str()) == Some("unavailable")
    }));
}

#[tokio::test]
async fn fake_run_produces_a_complete_bundle() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite"))
        .await
        .unwrap();
    let bundle_dir = dir.path().join("research/2026-11-04-fake-run");
    let job = store
        .enqueue(
            request("How do Tavily and Kagi compare?", Depth::Thorough),
            &bundle_dir,
            None,
        )
        .await
        .unwrap()
        .job;

    let worker = Worker::new(store.clone(), FakeProvider, worker_options());
    assert_eq!(worker.run_until_idle().await.unwrap(), 1);

    let job = store.require(job.id).await.unwrap();
    assert_eq!(job.status, JobStatus::Completed);
    assert!(job.lease_owner.is_none());

    for file in [
        "brief.md",
        "plan.v1.json",
        "plan.v1.md",
        "queries.jsonl",
        "sources.jsonl",
        "progress.v1.json",
        "report.md",
        "audit.md",
        "run.json",
    ] {
        assert!(bundle_dir.join(file).exists(), "missing {file}");
    }

    let report = std::fs::read_to_string(bundle_dir.join("report.md")).unwrap();
    assert!(report.contains("[S1]"));
    assert!(report.contains("## Sources"));

    let audit = std::fs::read_to_string(bundle_dir.join("audit.md")).unwrap();
    assert!(audit.contains("passed"));

    let metadata: RunMetadata =
        serde_json::from_slice(&std::fs::read(bundle_dir.join("run.json")).unwrap()).unwrap();
    assert_eq!(metadata.job_id, job.id.to_string());
    assert_eq!(metadata.provider, "fake");
    assert!(metadata.usage.claims > 0);
    assert!(metadata.audit.passed);

    let index = std::fs::read_to_string(dir.path().join("research/INDEX.md")).unwrap();
    assert!(index.contains("2026-11-04-fake-run"));
}

#[tokio::test]
async fn manual_review_pauses_until_the_plan_is_approved() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite"))
        .await
        .unwrap();
    let bundle_dir = dir.path().join("research/review");
    let mut request = request("A question that needs review", Depth::Quick);
    request.review = ReviewPolicy::Manual;
    let job = store.enqueue(request, &bundle_dir, None).await.unwrap().job;

    let worker = Worker::new(store.clone(), FakeProvider, worker_options());
    assert!(worker.run_once().await.unwrap());

    let paused = store.require(job.id).await.unwrap();
    assert_eq!(
        paused.status,
        JobStatus::AwaitingReview,
        "last_error={:?}",
        paused.last_error
    );
    assert!(bundle_dir.join("plan.v1.json").exists());
    assert!(!bundle_dir.join("report.md").exists());

    let bundle = Bundle::open(&bundle_dir).unwrap();
    bundle.mark_plan_approved().unwrap();
    store.approve_plan(job.id).await.unwrap();

    assert!(worker.run_once().await.unwrap());
    assert_eq!(
        store.require(job.id).await.unwrap().status,
        JobStatus::Completed
    );
}

/// Wraps the fake provider and injects a transient failure after a fixed
/// number of searches. Deleting the failure after `resume` exercises the
/// checkpoint path.
#[derive(Clone)]
struct FlakyProvider {
    inner: FakeProvider,
    searches: Arc<AtomicUsize>,
    fail_after: Arc<AtomicUsize>,
}

impl FlakyProvider {
    fn new(fail_after: usize) -> Self {
        Self {
            inner: FakeProvider,
            searches: Arc::new(AtomicUsize::new(0)),
            fail_after: Arc::new(AtomicUsize::new(fail_after)),
        }
    }

    fn stop_failing(&self) {
        self.fail_after.store(usize::MAX, Ordering::SeqCst);
    }
}

#[async_trait]
impl SearchProvider for FlakyProvider {
    fn id(&self) -> &'static str {
        "fake-flaky"
    }

    async fn search(
        &self,
        request: SearchRequest,
    ) -> Result<ProviderResponse<SearchResponse>, ProviderError> {
        let attempt = self.searches.fetch_add(1, Ordering::SeqCst) + 1;
        if attempt > self.fail_after.load(Ordering::SeqCst) {
            return Err(ProviderError::Transient("injected failure".to_owned()));
        }
        self.inner.search(request).await
    }

    async fn extract(
        &self,
        request: ExtractRequest,
    ) -> Result<ProviderResponse<ExtractResponse>, ProviderError> {
        self.inner.extract(request).await
    }
}

async fn queries_for(store: &JobStore, id: JobId) -> Vec<research_core::QueryRecord> {
    let job = store.require(id).await.unwrap();
    let bundle = Bundle::open(&job.output_dir).unwrap();
    bundle.load_queries().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn interrupted_collection_resumes_without_repeating_completed_subquestions() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite"))
        .await
        .unwrap();
    let bundle_dir = dir.path().join("research/resume");
    let job = store
        .enqueue(
            request("Resume this research job", Depth::Thorough),
            &bundle_dir,
            None,
        )
        .await
        .unwrap()
        .job;

    // The first subquestion issues two searches; the third search fails.
    let provider = FlakyProvider::new(3);
    let worker = Worker::new(store.clone(), provider.clone(), worker_options());
    assert!(worker.run_once().await.unwrap());

    let failed = store.require(job.id).await.unwrap();
    let events = store.events(job.id).await.unwrap();
    assert_eq!(
        failed.status,
        JobStatus::Failed,
        "events={events:?} last_error={:?}",
        failed.last_error
    );
    let error = failed.last_error.clone().unwrap_or_default();
    assert!(error.contains("injected failure"), "error was: {error}");

    let plan = Bundle::open(&bundle_dir)
        .unwrap()
        .load_plan()
        .unwrap()
        .unwrap();
    let first = plan.subquestions[0].id;
    let before = queries_for(&store, job.id)
        .await
        .into_iter()
        .filter(|query| query.subquestion_id == first)
        .count();
    assert_eq!(before, 2);

    provider.stop_failing();
    store.resume(job.id).await.unwrap();
    assert!(worker.run_once().await.unwrap());

    assert_eq!(
        store.require(job.id).await.unwrap().status,
        JobStatus::Completed
    );

    let after = queries_for(&store, job.id)
        .await
        .into_iter()
        .filter(|query| query.subquestion_id == first)
        .count();
    assert_eq!(after, before, "completed subquestions must not re-query");
}
