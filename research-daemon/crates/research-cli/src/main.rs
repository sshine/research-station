use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use chrono::{NaiveDate, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use research_core::{
    Config, Depth, Engine, ExtractDepth, ExtractFormat, ExtractRequest, Job, JobBudget, JobStatus,
    RawContentFormat, RedditMode, RedditSearchMode, RedditSearchRequest, RedditSort,
    RedditThreadRequest, ResearchRequest, ReviewPolicy, SearchDepth, SearchRequest, SearchTopic,
    TimeRange, slugify,
};
use research_llm::{LlmMessage, LlmProvider, LlmRequest};
use research_provider::{
    AnyReddit, ProviderResponse, RedditProvider, SearchProvider, TavilyProvider,
};
use research_service::{
    Worker, WorkerOptions, approve_plan, build_llm_provider, build_provider, build_reddit_provider,
    build_tavily_provider, resolve_api_key, resolve_llm_api_key, resolve_optional_reddit_api_key,
};
use research_store::JobStore;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "research",
    version,
    about = "Local research pipeline, job queue, and provider client"
)]
struct Cli {
    /// Path to researchd.toml. Defaults to $RESEARCHD_CONFIG, the repository
    /// config, then the XDG config directory.
    #[arg(long, global = true, env = "RESEARCHD_CONFIG")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect and exercise search providers.
    Provider {
        #[command(subcommand)]
        command: ProviderCommand,
    },
    /// Enqueue and inspect durable research jobs.
    Job {
        #[command(subcommand)]
        command: JobCommand,
    },
    /// Run the worker loop in this process.
    Serve(ServeArgs),
    /// Enqueue a question and run it immediately in this process.
    Run(RunArgs),
    /// Show the resolved configuration without secrets.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Check configuration, credentials, storage, and optionally the model.
    Doctor(DoctorArgs),
}

#[derive(Debug, Subcommand)]
enum ProviderCommand {
    /// Run a search through Tavily.
    Search(SearchArgs),
    /// Extract one or more URLs through Tavily.
    Extract(ExtractArgs),
    /// Verify that the configured provider, key, cache, and blob store work.
    Test,
    /// Search Reddit through the configured Reddit provider.
    RedditSearch(RedditSearchArgs),
    /// Fetch one Reddit thread and its comments.
    RedditThread(RedditThreadArgs),
    /// Verify the Reddit provider with one live read.
    RedditTest,
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    /// Print the resolved configuration.
    Show,
}

#[derive(Debug, Subcommand)]
enum JobCommand {
    /// Add a research question to the durable queue.
    Enqueue(EnqueueArgs),
    /// List queued and finished jobs, newest first.
    List(ListArgs),
    /// Show one job and its transition history.
    Show {
        /// Job ID.
        id: String,
        /// Print the job and its events as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Request cancellation. Queued jobs stop immediately; active jobs stop at
    /// their next checkpoint.
    Cancel {
        /// Job ID.
        id: String,
        /// Print the updated job as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Requeue an interrupted or failed job.
    Resume {
        /// Job ID.
        id: String,
        /// Print the updated job as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Approve a plan that paused for manual review.
    Approve {
        /// Job ID.
        id: String,
        /// Print the updated job as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Wait for a job to finish or pause for review.
    Wait(WaitArgs),
    /// Print a finished job's report.
    Report {
        /// Job ID.
        id: String,
    },
    /// Print a job's transition history.
    Events {
        /// Job ID.
        id: String,
        /// Print the events as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Print the agent transcript, including model prompts and tool results.
    Transcript {
        /// Job ID.
        id: String,
        /// Print the transcript as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Claim the oldest queued job. Intended for workers and integration tests.
    #[command(hide = true)]
    Claim {
        /// Lease owner recorded on the job.
        #[arg(long, default_value = "cli-worker")]
        owner: String,
        /// Lease length in seconds. Defaults to the configured value.
        #[arg(long)]
        lease_seconds: Option<u64>,
        /// Print the claimed job as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Move jobs with expired leases to interrupted. Intended for workers.
    #[command(hide = true)]
    Reap,
}

#[derive(Debug, Args)]
struct EnqueueArgs {
    /// Research question.
    question: String,

    #[arg(long, value_enum, default_value_t = ResearchDepthArg::Thorough)]
    depth: ResearchDepthArg,

    /// Research engine. Defaults to the iterative agent.
    #[arg(long, value_enum, default_value_t = EngineArg::Agent)]
    engine: EngineArg,

    /// Pause for human plan review before collecting evidence.
    #[arg(long)]
    review: bool,

    /// Reddit policy: auto, always, or never.
    #[arg(long, value_enum, default_value_t = RedditModeArg::Auto)]
    reddit: RedditModeArg,

    /// Query budget for this job. Defaults to the depth's allowance.
    #[arg(long)]
    max_queries: Option<u32>,

    /// Credit budget for this job.
    #[arg(long)]
    max_credits: Option<u64>,

    /// Bundle directory. Defaults to research/<date>-<slug>.
    #[arg(long)]
    output_dir: Option<PathBuf>,

    /// Return the existing job when this key was already used.
    #[arg(long)]
    idempotency_key: Option<String>,

    /// Print the job as JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ListArgs {
    /// Only show jobs in this state.
    #[arg(long, value_enum)]
    status: Option<StatusArg>,

    #[arg(long, default_value_t = 20)]
    limit: u32,

    /// Print the jobs as JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ServeArgs {
    /// Use the deterministic fake provider instead of Tavily.
    #[arg(long)]
    fake: bool,

    /// Use the deterministic fake model instead of OpenCode Go.
    #[arg(long)]
    fake_llm: bool,

    /// Process the current queue once, then exit.
    #[arg(long)]
    once: bool,

    /// Lease owner name shown on jobs.
    #[arg(long, default_value = "research-cli")]
    owner: String,

    /// Idle poll interval in seconds. Defaults to the configured value.
    #[arg(long)]
    poll_seconds: Option<u64>,
}

#[derive(Debug, Args)]
struct RunArgs {
    /// Research question.
    question: String,

    #[arg(long, value_enum, default_value_t = ResearchDepthArg::Thorough)]
    depth: ResearchDepthArg,

    /// Research engine. Defaults to the iterative agent.
    #[arg(long, value_enum, default_value_t = EngineArg::Agent)]
    engine: EngineArg,

    /// Pause for plan approval before collecting evidence.
    #[arg(long)]
    review: bool,

    /// Reddit policy: auto, always, or never.
    #[arg(long, value_enum, default_value_t = RedditModeArg::Auto)]
    reddit: RedditModeArg,

    /// Query budget for this job.
    #[arg(long)]
    max_queries: Option<u32>,

    /// Credit budget for this job.
    #[arg(long)]
    max_credits: Option<u64>,

    /// Bundle directory. Defaults to research/<date>-<slug>.
    #[arg(long)]
    output_dir: Option<PathBuf>,

    /// Use the deterministic fake provider instead of Tavily.
    #[arg(long)]
    fake: bool,

    /// Use the deterministic fake model instead of OpenCode Go.
    #[arg(long)]
    fake_llm: bool,

    /// Print the final job as JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct DoctorArgs {
    /// Make a minimal model request to verify connectivity.
    #[arg(long)]
    probe: bool,

    /// Use the deterministic fake model for the probe.
    #[arg(long)]
    fake_llm: bool,

    /// Exit nonzero when credentials or storage checks fail.
    #[arg(long)]
    strict: bool,
}

#[derive(Debug, Args)]
struct WaitArgs {
    /// Job ID.
    id: String,

    /// Give up after this many seconds.
    #[arg(long, default_value_t = 1800)]
    timeout_seconds: u64,

    /// Print the final job as JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct SearchArgs {
    /// Search query.
    query: String,

    #[arg(long, value_enum, default_value_t = DepthArg::Basic)]
    depth: DepthArg,

    #[arg(long, default_value_t = 10)]
    max_results: u8,

    #[arg(long, value_enum, default_value_t = TopicArg::General)]
    topic: TopicArg,

    #[arg(long, value_enum)]
    time_range: Option<TimeRangeArg>,

    #[arg(long)]
    start_date: Option<NaiveDate>,

    #[arg(long)]
    end_date: Option<NaiveDate>,

    #[arg(long = "include-domain")]
    include_domains: Vec<String>,

    #[arg(long = "exclude-domain")]
    exclude_domains: Vec<String>,

    #[arg(long)]
    country: Option<String>,

    /// Ask Tavily to return cleaned page content in markdown.
    #[arg(long)]
    raw_content: bool,

    /// Ask Tavily to include its best-estimate publish date per result.
    #[arg(long)]
    include_published_date: bool,

    /// Drop results whose publish date is outside the requested window.
    #[arg(long)]
    filter_by_published_date: bool,

    /// Print the full normalized response as JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ExtractArgs {
    /// One or more HTTPS URLs. Tavily accepts at most 20 per request.
    urls: Vec<String>,

    /// Rerank extracted chunks against this intent.
    #[arg(long)]
    query: Option<String>,

    #[arg(long, value_enum, default_value_t = ExtractDepthArg::Basic)]
    depth: ExtractDepthArg,

    #[arg(long, value_enum, default_value_t = ExtractFormatArg::Markdown)]
    format: ExtractFormatArg,

    #[arg(long)]
    images: bool,

    #[arg(long)]
    timeout: Option<f64>,

    /// Print the full normalized response as JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct RedditSearchArgs {
    /// Reddit search query.
    query: String,

    /// Restrict the search to one subreddit (no r/ prefix needed).
    #[arg(long)]
    subreddit: Option<String>,

    #[arg(long, value_enum, default_value_t = RedditSortArg::Relevance)]
    sort: RedditSortArg,

    /// Search post titles/selftext or comment bodies.
    #[arg(long, value_enum, default_value_t = RedditSearchModeArg::Posts)]
    mode: RedditSearchModeArg,

    #[arg(long, value_enum)]
    time_range: Option<TimeRangeArg>,

    #[arg(long, default_value_t = 25)]
    limit: u8,

    /// Keep only posts with at least this score.
    #[arg(long)]
    min_score: Option<u32>,

    /// Fetch and store the top comments for the leading results.
    #[arg(long)]
    comments: bool,

    #[arg(long, default_value_t = 20)]
    comment_limit: u8,

    /// Print the full normalized response as JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct RedditThreadArgs {
    /// Reddit post id or full URL.
    id_or_url: String,

    #[arg(long, default_value_t = 100)]
    limit: u8,

    #[arg(long, value_enum, default_value_t = RedditSortArg::Top)]
    sort: RedditSortArg,

    /// Print the full normalized response as JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DepthArg {
    Fast,
    #[value(name = "ultra-fast")]
    UltraFast,
    Basic,
    Advanced,
}

impl From<DepthArg> for SearchDepth {
    fn from(value: DepthArg) -> Self {
        match value {
            DepthArg::Fast => Self::Fast,
            DepthArg::UltraFast => Self::UltraFast,
            DepthArg::Basic => Self::Basic,
            DepthArg::Advanced => Self::Advanced,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum EngineArg {
    Agent,
    Deterministic,
}

impl From<EngineArg> for Engine {
    fn from(value: EngineArg) -> Self {
        match value {
            EngineArg::Agent => Self::Agent,
            EngineArg::Deterministic => Self::Deterministic,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum RedditSortArg {
    Relevance,
    Hot,
    New,
    Top,
    Comments,
}

impl From<RedditSortArg> for RedditSort {
    fn from(value: RedditSortArg) -> Self {
        match value {
            RedditSortArg::Relevance => Self::Relevance,
            RedditSortArg::Hot => Self::Hot,
            RedditSortArg::New => Self::New,
            RedditSortArg::Top => Self::Top,
            RedditSortArg::Comments => Self::Comments,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum RedditSearchModeArg {
    Posts,
    Comments,
}

impl From<RedditSearchModeArg> for RedditSearchMode {
    fn from(value: RedditSearchModeArg) -> Self {
        match value {
            RedditSearchModeArg::Posts => Self::Posts,
            RedditSearchModeArg::Comments => Self::Comments,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum RedditModeArg {
    Auto,
    Always,
    Never,
}

impl From<RedditModeArg> for RedditMode {
    fn from(value: RedditModeArg) -> Self {
        match value {
            RedditModeArg::Auto => Self::Auto,
            RedditModeArg::Always => Self::Always,
            RedditModeArg::Never => Self::Never,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ResearchDepthArg {
    Quick,
    Thorough,
    Exhaustive,
}

impl From<ResearchDepthArg> for Depth {
    fn from(value: ResearchDepthArg) -> Self {
        match value {
            ResearchDepthArg::Quick => Self::Quick,
            ResearchDepthArg::Thorough => Self::Thorough,
            ResearchDepthArg::Exhaustive => Self::Exhaustive,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum StatusArg {
    Queued,
    Planning,
    #[value(name = "awaiting-review")]
    AwaitingReview,
    Running,
    Synthesizing,
    Auditing,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

impl From<StatusArg> for JobStatus {
    fn from(value: StatusArg) -> Self {
        match value {
            StatusArg::Queued => Self::Queued,
            StatusArg::Planning => Self::Planning,
            StatusArg::AwaitingReview => Self::AwaitingReview,
            StatusArg::Running => Self::Running,
            StatusArg::Synthesizing => Self::Synthesizing,
            StatusArg::Auditing => Self::Auditing,
            StatusArg::Completed => Self::Completed,
            StatusArg::Failed => Self::Failed,
            StatusArg::Cancelled => Self::Cancelled,
            StatusArg::Interrupted => Self::Interrupted,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TopicArg {
    General,
    News,
    Finance,
}

impl From<TopicArg> for SearchTopic {
    fn from(value: TopicArg) -> Self {
        match value {
            TopicArg::General => Self::General,
            TopicArg::News => Self::News,
            TopicArg::Finance => Self::Finance,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TimeRangeArg {
    Day,
    Week,
    Month,
    Year,
}

impl From<TimeRangeArg> for TimeRange {
    fn from(value: TimeRangeArg) -> Self {
        match value {
            TimeRangeArg::Day => Self::Day,
            TimeRangeArg::Week => Self::Week,
            TimeRangeArg::Month => Self::Month,
            TimeRangeArg::Year => Self::Year,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExtractDepthArg {
    Basic,
    Advanced,
}

impl From<ExtractDepthArg> for ExtractDepth {
    fn from(value: ExtractDepthArg) -> Self {
        match value {
            ExtractDepthArg::Basic => Self::Basic,
            ExtractDepthArg::Advanced => Self::Advanced,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExtractFormatArg {
    Markdown,
    Text,
}

impl From<ExtractFormatArg> for ExtractFormat {
    fn from(value: ExtractFormatArg) -> Self {
        match value {
            ExtractFormatArg::Markdown => Self::Markdown,
            ExtractFormatArg::Text => Self::Text,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();
    let config_path = Config::discover(cli.config);

    match cli.command {
        Command::Config {
            command: ConfigCommand::Show,
        } => show_config(config_path),
        Command::Provider { command } => run_provider_command(config_path, command).await,
        Command::Job { command } => run_job_command(config_path, command).await,
        Command::Serve(args) => run_serve(config_path, args).await,
        Command::Run(args) => run_research(config_path, args).await,
        Command::Doctor(args) => run_doctor(config_path, args).await,
    }
}

async fn run_job_command(config_path: Option<PathBuf>, command: JobCommand) -> anyhow::Result<()> {
    let config = Config::load(config_path.as_deref()).context("loading configuration")?;
    let store = JobStore::open(config.database_path())
        .await
        .context("opening job database")?;

    match command {
        JobCommand::Enqueue(args) => run_job_enqueue(&store, args).await,
        JobCommand::List(args) => run_job_list(&store, args).await,
        JobCommand::Show { id, json } => {
            let id = parse_job_id(&id)?;
            let job = store.require(id).await?;
            let events = store.events(id).await?;

            if json {
                print_pretty_json(&serde_json::json!({ "job": job, "events": events }))
            } else {
                print_job(&job);
                println!("\nhistory:");
                for event in events {
                    let detail = event
                        .detail
                        .map(|detail| format!(" ({detail})"))
                        .unwrap_or_default();
                    println!("  {}  {}{}", event.at.to_rfc3339(), event.status, detail);
                }
                Ok(())
            }
        }
        JobCommand::Cancel { id, json } => {
            let id = parse_job_id(&id)?;
            let job = store.cancel(id).await?;
            let label = if job.status == JobStatus::Cancelled {
                "cancelled"
            } else {
                "cancellation requested for"
            };
            if json {
                print_pretty_json(&job)
            } else {
                println!("{label} {}", job.id);
                print_job(&job);
                Ok(())
            }
        }
        JobCommand::Resume { id, json } => {
            let id = parse_job_id(&id)?;
            let job = store.resume(id).await?;
            if json {
                print_pretty_json(&job)
            } else {
                println!("requeued {}", job.id);
                print_job(&job);
                Ok(())
            }
        }
        JobCommand::Approve { id, json } => {
            let id = parse_job_id(&id)?;
            let job = approve_plan(&store, id).await?;
            if json {
                print_pretty_json(&job)
            } else {
                println!("plan approved and requeued {}", job.id);
                print_job(&job);
                Ok(())
            }
        }
        JobCommand::Wait(args) => run_job_wait(&store, args).await,
        JobCommand::Report { id } => {
            let job = store.require(parse_job_id(&id)?).await?;
            let path = job.output_dir.join("report.md");
            let report = std::fs::read_to_string(&path).with_context(|| {
                format!(
                    "no report at {}; run `research job wait {}` first",
                    path.display(),
                    job.id
                )
            })?;
            println!("{report}");
            Ok(())
        }
        JobCommand::Events { id, json } => {
            let events = store.events(parse_job_id(&id)?).await?;
            if json {
                print_pretty_json(&events)
            } else {
                for event in events {
                    let detail = event
                        .detail
                        .map(|detail| format!(" ({detail})"))
                        .unwrap_or_default();
                    println!("{}  {}{}", event.at.to_rfc3339(), event.status, detail);
                }
                Ok(())
            }
        }
        JobCommand::Transcript { id, json } => {
            let id = parse_job_id(&id)?;
            let transcripts = research_store::AgentStore::open(config.database_path())
                .await
                .context("opening agent transcript store")?;
            let run = transcripts.run(id).await?;
            let events = transcripts.events(id).await?;
            if json {
                print_pretty_json(&serde_json::json!({ "run": run, "events": events }))
            } else {
                if let Some(run) = &run {
                    println!(
                        "run: {} {} / {} ({}) started {}",
                        run.engine,
                        run.provider,
                        run.model,
                        run.status,
                        run.started_at.to_rfc3339()
                    );
                    if let Some(reason) = &run.stop_reason {
                        println!("stop: {reason}");
                    }
                }
                for event in events {
                    println!("\n#{} {} {}", event.seq, event.kind, event.at.to_rfc3339());
                    println!("{}", serde_json::to_string_pretty(&event.payload)?);
                }
                Ok(())
            }
        }
        JobCommand::Claim {
            owner,
            lease_seconds,
            json,
        } => {
            let lease = Duration::from_secs(lease_seconds.unwrap_or(config.jobs.lease_seconds));
            match store.claim(&owner, lease).await? {
                Some(job) if json => print_pretty_json(&job),
                Some(job) => {
                    println!("claimed {}", job.id);
                    print_job(&job);
                    Ok(())
                }
                None => {
                    println!("no queued jobs");
                    Ok(())
                }
            }
        }
        JobCommand::Reap => {
            let reaped = store.reap_expired_leases(Utc::now()).await?;
            if reaped.is_empty() {
                println!("no expired leases");
            } else {
                for job in reaped {
                    println!("{} {}", job.status, job.id);
                }
            }
            Ok(())
        }
    }
}

async fn run_job_enqueue(store: &JobStore, args: EnqueueArgs) -> anyhow::Result<()> {
    let mut request = ResearchRequest::new(args.question);
    request.engine = args.engine.into();
    request.depth = args.depth.into();
    request.budget = JobBudget::for_depth(request.depth);
    request.review = if args.review {
        ReviewPolicy::Manual
    } else {
        ReviewPolicy::Automatic
    };
    request.reddit = args.reddit.into();
    if let Some(max_queries) = args.max_queries {
        request.budget.max_queries = max_queries;
    }
    if let Some(max_credits) = args.max_credits {
        request.budget.max_credits = max_credits;
    }

    let output_dir = args
        .output_dir
        .unwrap_or_else(|| default_output_dir(&request.question));
    request.output_dir = Some(output_dir.clone());

    let outcome = store
        .enqueue(request, output_dir, args.idempotency_key.as_deref())
        .await?;

    if args.json {
        return print_pretty_json(&outcome.job);
    }

    println!(
        "{} {}",
        if outcome.created {
            "enqueued"
        } else {
            "existing job"
        },
        outcome.job.id
    );
    print_job(&outcome.job);
    Ok(())
}

async fn run_job_list(store: &JobStore, args: ListArgs) -> anyhow::Result<()> {
    let jobs = store.list(args.status.map(Into::into), args.limit).await?;

    if args.json {
        return print_pretty_json(&jobs);
    }
    if jobs.is_empty() {
        println!("no jobs");
        return Ok(());
    }

    for job in jobs {
        println!(
            "{}  {:<15}  {:>2}  {}  {}",
            job.id,
            job.status.as_str(),
            job.attempts,
            job.created_at.to_rfc3339(),
            truncate(&job.request.question, 70)
        );
    }
    Ok(())
}

async fn run_serve(config_path: Option<PathBuf>, args: ServeArgs) -> anyhow::Result<()> {
    let config = Config::load(config_path.as_deref()).context("loading configuration")?;
    let store = JobStore::open(config.database_path())
        .await
        .context("opening job database")?;

    let mut options = WorkerOptions::from_config(&config, args.owner);
    if let Some(seconds) = args.poll_seconds {
        options.poll_interval = Duration::from_secs(seconds.max(1));
    }

    let provider = build_provider(&config, args.fake)
        .await
        .context("creating provider")?;
    let reddit = build_reddit_provider(&config, args.fake)
        .await
        .context("creating Reddit provider")?;
    let reddit_label = reddit.as_ref().map(RedditProvider::id).unwrap_or("off");
    let llm = build_llm_provider(&config, args.fake_llm).context("creating model provider")?;
    let transcripts = research_store::AgentStore::open(config.database_path())
        .await
        .context("opening agent transcript store")?;
    let worker = Worker::with_agent_reddit(store, provider, reddit, llm, transcripts, options);
    worker.reap_expired().await?;

    if args.once {
        let processed = worker.run_until_idle().await?;
        println!("processed {processed} job(s)");
        return Ok(());
    }

    println!(
        "research worker: owner={} provider={} reddit={} database={}",
        worker.options().owner,
        if args.fake { "fake" } else { "tavily" },
        reddit_label,
        config.database_path().display()
    );
    worker
        .run_forever(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    println!("stopped");
    Ok(())
}

async fn run_research(config_path: Option<PathBuf>, args: RunArgs) -> anyhow::Result<()> {
    let config = Config::load(config_path.as_deref()).context("loading configuration")?;
    let store = JobStore::open(config.database_path())
        .await
        .context("opening job database")?;
    let provider = build_provider(&config, args.fake)
        .await
        .context("creating provider")?;
    let reddit = build_reddit_provider(&config, args.fake)
        .await
        .context("creating Reddit provider")?;
    let llm = build_llm_provider(&config, args.fake_llm).context("creating model provider")?;
    let transcripts = research_store::AgentStore::open(config.database_path())
        .await
        .context("opening agent transcript store")?;

    let mut request = ResearchRequest::new(args.question);
    request.engine = args.engine.into();
    request.depth = args.depth.into();
    request.budget = JobBudget::for_depth(request.depth);
    request.review = if args.review {
        ReviewPolicy::Manual
    } else {
        ReviewPolicy::Automatic
    };
    request.reddit = args.reddit.into();
    if let Some(max_queries) = args.max_queries {
        request.budget.max_queries = max_queries;
    }
    if let Some(max_credits) = args.max_credits {
        request.budget.max_credits = max_credits;
    }

    let output_dir = args
        .output_dir
        .unwrap_or_else(|| default_output_dir(&request.question));
    request.output_dir = Some(output_dir.clone());

    let job = store.enqueue(request, &output_dir, None).await?.job;
    let mut options = WorkerOptions::from_config(&config, "research-run");
    options.poll_interval = Duration::from_millis(200);

    let worker =
        Worker::with_agent_reddit(store.clone(), provider, reddit, llm, transcripts, options);

    println!("running {} in {}", job.id, job.output_dir.display());
    let Some(outcome) = worker.run_job(job.id).await? else {
        anyhow::bail!(
            "job {} is already claimed by another worker; use `research job wait {}`",
            job.id,
            job.id
        );
    };
    let finished = store.require(job.id).await?;

    if args.json {
        print_pretty_json(&finished)?;
    } else if outcome.status == JobStatus::Completed {
        println!("completed {}", job.id);
        println!("report: {}", outcome.report_path.display());
        println!("sources: {}  claims: {}", outcome.sources, outcome.claims);
        if let Some(reason) = &outcome.stop_reason {
            println!("stop: {reason}");
        }
        if outcome.rounds > 0 {
            println!("agent rounds: {}", outcome.rounds);
        }
    } else if outcome.status == JobStatus::AwaitingReview {
        println!(
            "plan ready for review: {}",
            job.output_dir.join("plan.v1.md").display()
        );
        println!("approve with: research job approve {}", job.id);
    }

    if !matches!(
        outcome.status,
        JobStatus::Completed | JobStatus::AwaitingReview
    ) {
        let reason = finished
            .last_error
            .as_deref()
            .unwrap_or("no error recorded");
        anyhow::bail!("job {} ended as {}: {reason}", job.id, outcome.status);
    }

    Ok(())
}

async fn run_job_wait(store: &JobStore, args: WaitArgs) -> anyhow::Result<()> {
    let id = parse_job_id(&args.id)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(args.timeout_seconds.max(1));
    let mut last = None;

    loop {
        let job = store.require(id).await?;
        if last != Some(job.status) {
            println!("{}  {}", job.status, job.id);
            last = Some(job.status);
        }

        if job.status.is_terminal()
            || matches!(job.status, JobStatus::Failed | JobStatus::AwaitingReview)
        {
            if args.json {
                print_pretty_json(&job)?;
            } else if job.status == JobStatus::AwaitingReview {
                println!(
                    "plan is ready; approve with `research job approve {}`",
                    job.id
                );
            } else if job.status == JobStatus::Completed {
                println!("report: {}", job.output_dir.join("report.md").display());
            } else if let Some(error) = &job.last_error {
                println!("error: {error}");
            }
            return Ok(());
        }

        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for job {id}");
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

fn print_job(job: &Job) {
    println!("id:         {}", job.id);
    println!("status:     {}", job.status);
    println!("engine:     {}", job.request.engine.as_str());
    println!("depth:      {}", job.request.depth.as_str());
    println!(
        "review:     {}",
        match job.request.review {
            ReviewPolicy::Automatic => "automatic",
            ReviewPolicy::Manual => "manual",
        }
    );
    println!("output:     {}", job.output_dir.display());
    println!("question:   {}", job.request.question);
    println!("created:    {}", job.created_at.to_rfc3339());
    println!("updated:    {}", job.updated_at.to_rfc3339());
    println!("attempts:   {}", job.attempts);

    if let Some(owner) = &job.lease_owner {
        match job.lease_expires_at {
            Some(expires) => println!("lease:      {owner} until {}", expires.to_rfc3339()),
            None => println!("lease:      {owner}"),
        }
    }
    if job.cancel_requested {
        println!("cancelling: requested");
    }
    if let Some(error) = &job.last_error {
        println!("error:      {error}");
    }
    if let Some(key) = &job.idempotency_key {
        println!("idempotent: {key}");
    }
}

fn parse_job_id(value: &str) -> anyhow::Result<research_core::JobId> {
    value
        .parse()
        .with_context(|| format!("invalid job ID: {value}"))
}

fn default_output_dir(question: &str) -> PathBuf {
    let date = Utc::now().format("%Y-%m-%d");
    PathBuf::from("research").join(format!("{date}-{}", slugify(question, 48)))
}

async fn run_doctor(config_path: Option<PathBuf>, args: DoctorArgs) -> anyhow::Result<()> {
    let discovered = Config::discover(config_path);
    let config = Config::load(discovered.as_deref()).context("loading configuration")?;
    let mut warnings = Vec::new();

    let config_label = discovered
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "built-in defaults".to_owned());
    println!("config:       ok ({config_label})");

    match resolve_api_key(&config) {
        Ok(_) => println!("tavily key:   ok ({})", config.provider.tavily.api_key_env),
        Err(err) => {
            println!(
                "tavily key:   missing ({})",
                config.provider.tavily.api_key_env
            );
            warnings.push(err.to_string());
        }
    }

    let reddit_label = match resolve_optional_reddit_api_key(&config) {
        Ok(Some(_)) => format!("ok ({})", config.provider.reddit.api_key_env),
        Ok(None) if config.provider.reddit.mode == RedditMode::Never => {
            "disabled by config".to_owned()
        }
        Ok(None) => format!(
            "missing ({}, optional; web fallback)",
            config.provider.reddit.api_key_env
        ),
        Err(err) => {
            warnings.push(err.to_string());
            "failed to resolve".to_owned()
        }
    };
    println!("reddit key:   {reddit_label}");
    println!(
        "reddit:       mode={} backend={} base={}",
        config.provider.reddit.mode.as_str(),
        config.provider.reddit.backend,
        config.provider.reddit.base_url
    );

    let llm_names = config.llm.api_key_names().join(" or ");
    match resolve_llm_api_key(&config) {
        Ok(_) => println!("llm key:      ok ({llm_names})"),
        Err(err) => {
            println!("llm key:      missing ({llm_names})");
            warnings.push(err.to_string());
        }
    }
    println!(
        "llm:          {} / {}",
        config.llm.provider, config.llm.model
    );
    println!("llm base:     {}", config.llm.base_url);

    match JobStore::open(config.database_path()).await {
        Ok(_) => println!("database:     ok ({})", config.database_path().display()),
        Err(err) => {
            println!(
                "database:     failed ({})",
                config.database_path().display()
            );
            warnings.push(format!("database: {err}"));
        }
    }

    match std::fs::create_dir_all(config.blob_dir()) {
        Ok(()) => println!("blobs:        ok ({})", config.blob_dir().display()),
        Err(err) => {
            println!("blobs:        failed ({})", config.blob_dir().display());
            warnings.push(format!("blob directory: {err}"));
        }
    }

    if args.probe {
        match build_llm_provider(&config, args.fake_llm) {
            Ok(provider) => {
                let request = LlmRequest::new(vec![
                    LlmMessage::system("You are a research-station health check."),
                    LlmMessage::user("Reply with exactly OK."),
                ])
                .with_temperature(0.0)
                .with_max_tokens(16);

                match provider.complete(request).await {
                    Ok(response) => {
                        println!(
                            "llm probe:    ok ({} / {}): {}",
                            response.provider,
                            response.model,
                            response.content.trim()
                        );
                        println!(
                            "llm usage:    prompt={} completion={} total={} cost={}",
                            response.usage.prompt_tokens,
                            response.usage.completion_tokens,
                            response.usage.total_tokens,
                            response
                                .usage
                                .cost_usd
                                .map(|cost| format!("{cost:.6}"))
                                .unwrap_or_else(|| "unknown".to_owned())
                        );
                    }
                    Err(err) => {
                        println!("llm probe:    failed");
                        warnings.push(format!("llm probe: {err}"));
                    }
                }
            }
            Err(err) => {
                println!("llm probe:    failed");
                warnings.push(format!("llm probe: {err}"));
            }
        }
    }

    if warnings.is_empty() {
        return Ok(());
    }

    println!("\n{} warning(s):", warnings.len());
    for warning in &warnings {
        println!("  - {warning}");
    }
    if args.strict {
        anyhow::bail!("doctor found {} problem(s)", warnings.len());
    }

    println!("Run with `--strict` to fail on warnings.");
    Ok(())
}

async fn run_provider_command(
    config_path: Option<PathBuf>,
    command: ProviderCommand,
) -> anyhow::Result<()> {
    let config = Config::load(config_path.as_deref()).context("loading configuration")?;
    match command {
        ProviderCommand::Search(args) => {
            let provider = build_tavily_provider(&config)
                .await
                .context("creating Tavily provider")?;
            run_search(&provider, args).await
        }
        ProviderCommand::Extract(args) => {
            let provider = build_tavily_provider(&config)
                .await
                .context("creating Tavily provider")?;
            run_extract(&provider, args).await
        }
        ProviderCommand::Test => {
            let provider = build_tavily_provider(&config)
                .await
                .context("creating Tavily provider")?;
            run_test(&provider).await
        }
        ProviderCommand::RedditSearch(args) => {
            let reddit = build_reddit_provider(&config, false)
                .await
                .context("creating Reddit provider")?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Reddit is not configured; store {} with `just secrets-set-reddit` or export it",
                        config.provider.reddit.api_key_env
                    )
                })?;
            run_reddit_search(&reddit, args).await
        }
        ProviderCommand::RedditThread(args) => {
            let reddit = build_reddit_provider(&config, false)
                .await
                .context("creating Reddit provider")?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Reddit is not configured; store {} with `just secrets-set-reddit` or export it",
                        config.provider.reddit.api_key_env
                    )
                })?;
            run_reddit_thread(&reddit, args).await
        }
        ProviderCommand::RedditTest => {
            let reddit = build_reddit_provider(&config, false)
                .await
                .context("creating Reddit provider")?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Reddit is not configured; store {} with `just secrets-set-reddit` or export it",
                        config.provider.reddit.api_key_env
                    )
                })?;
            run_reddit_test(&reddit).await
        }
    }
}

async fn run_search(provider: &TavilyProvider, args: SearchArgs) -> anyhow::Result<()> {
    let mut request = SearchRequest::new(args.query)
        .with_depth(args.depth.into())
        .with_max_results(args.max_results);
    request.topic = args.topic.into();
    request.time_range = args.time_range.map(Into::into);
    request.start_date = args.start_date;
    request.end_date = args.end_date;
    request.include_domains = args.include_domains;
    request.exclude_domains = args.exclude_domains;
    request.country = args.country;
    request.include_raw_content = args.raw_content.then_some(RawContentFormat::Markdown);
    request.include_published_date = args.include_published_date;
    request.filter_by_published_date = args.filter_by_published_date;

    let response = provider.search(request).await?;

    if args.json {
        print_json(&response)?;
        return Ok(());
    }

    print_header(&response);
    println!("query: {}", response.data.query);
    if let Some(answer) = &response.data.answer {
        println!("\nanswer:\n{answer}");
    }

    for (index, result) in response.data.results.iter().enumerate() {
        println!(
            "\n{}. {}",
            index + 1,
            display_title(result.title.as_str(), &result.url)
        );
        println!("   {}", result.url);
        if let Some(score) = result.score {
            println!("   score: {score:.3}");
        }
        if let Some(published) = &result.published_date {
            println!("   published: {published}");
        }
        let preview = truncate(&result.content, 400);
        if !preview.is_empty() {
            println!("   {preview}");
        }
    }

    if let Some(hash) = &response.raw_hash {
        println!("\nraw response blob: {hash}");
    }

    Ok(())
}

async fn run_extract(provider: &TavilyProvider, args: ExtractArgs) -> anyhow::Result<()> {
    let mut request = ExtractRequest::new(args.urls);
    request.query = args.query;
    request.extract_depth = args.depth.into();
    request.format = args.format.into();
    request.include_images = args.images;
    request.timeout_seconds = args.timeout;

    let response = provider.extract(request).await?;

    if args.json {
        print_json(&response)?;
        return Ok(());
    }

    print_header(&response);
    for result in &response.data.results {
        println!("\n== {}", result.url);
        println!("{}", truncate(&result.raw_content, 1_000));
    }
    for failed in &response.data.failed_results {
        println!("\nFAILED {}: {}", failed.url, failed.error);
    }
    if let Some(hash) = &response.raw_hash {
        println!("\nraw response blob: {hash}");
    }

    Ok(())
}

async fn run_test(provider: &TavilyProvider) -> anyhow::Result<()> {
    let query = std::env::var("TAVILY_TEST_QUERY")
        .ok()
        .filter(|query| !query.trim().is_empty())
        .unwrap_or_else(|| "Tavily API connectivity test".to_owned());
    let request = SearchRequest::new(query)
        .with_max_results(1)
        .with_depth(SearchDepth::Basic);
    let response = provider.search(request).await?;

    println!(
        "ok: provider={} results={}",
        response.provider,
        response.data.results.len()
    );
    println!(
        "cache_hit={} credits={} latency_ms={}",
        response.cache_hit,
        response
            .credits
            .map(|credits| credits.to_string())
            .unwrap_or_else(|| "unknown".to_owned()),
        response.latency_ms
    );
    Ok(())
}

async fn run_reddit_search(provider: &AnyReddit, args: RedditSearchArgs) -> anyhow::Result<()> {
    let mut request = RedditSearchRequest::new(args.query);
    request.subreddit = args.subreddit;
    request.sort = args.sort.into();
    request.mode = args.mode.into();
    request.time_range = args.time_range.map(Into::into);
    request.max_results = Some(args.limit.clamp(1, 100));
    request.min_score = args.min_score;
    request.include_comments = Some(args.comments);
    request.comment_limit = args.comments.then_some(args.comment_limit.clamp(1, 100));

    let response = provider.search(request).await?;

    if args.json {
        print_json(&response)?;
        return Ok(());
    }

    print_header(&response);
    println!("query: {}", response.data.query);
    for (index, result) in response.data.results.iter().enumerate() {
        println!(
            "\n{}. {}",
            index + 1,
            display_title(result.title.as_str(), &result.url)
        );
        println!("   {}", result.url);
        if let Some(score) = result.score {
            println!("   score: {score:.0}");
        }
        if let Some(published) = &result.published_date {
            println!("   created: {published}");
        }
        let preview = truncate(&result.content, 500);
        if !preview.is_empty() {
            println!("   {preview}");
        }
    }
    if let Some(hash) = &response.raw_hash {
        println!("\nraw response blob: {hash}");
    }
    Ok(())
}

async fn run_reddit_thread(provider: &AnyReddit, args: RedditThreadArgs) -> anyhow::Result<()> {
    let request = RedditThreadRequest {
        id_or_url: args.id_or_url,
        limit: Some(args.limit.clamp(1, 100)),
        sort: args.sort.into(),
    };
    let response = provider.thread(request).await?;

    if args.json {
        print_json(&response)?;
        return Ok(());
    }

    print_header(&response);
    for result in &response.data.results {
        println!("\n# {}", result.title);
        println!("{}", result.url);
        println!("{}", truncate(&result.content, 2_000));
    }
    if let Some(hash) = &response.raw_hash {
        println!("\nraw response blob: {hash}");
    }
    Ok(())
}

async fn run_reddit_test(provider: &AnyReddit) -> anyhow::Result<()> {
    let request = RedditSearchRequest {
        subreddit: Some("test".to_owned()),
        max_results: Some(1),
        ..RedditSearchRequest::new("research-station connectivity test")
    };
    let response = provider.search(request).await?;
    println!(
        "ok: provider={} results={} cache_hit={} credits={}",
        response.provider,
        response.data.results.len(),
        response.cache_hit,
        response
            .credits
            .map(|credits| credits.to_string())
            .unwrap_or_else(|| "unknown".to_owned())
    );
    Ok(())
}

fn show_config(config_path: Option<PathBuf>) -> anyhow::Result<()> {
    let config = Config::load(config_path.as_deref())?;
    println!("{}", toml::to_string_pretty(&config)?);
    println!("# resolved database:  {}", config.database_path().display());
    println!("# resolved blob dir:  {}", config.blob_dir().display());
    println!(
        "# API key variable:    {}",
        config.provider.tavily.api_key_env
    );
    println!(
        "# Reddit key variable: {} (optional)",
        config.provider.reddit.api_key_env
    );
    Ok(())
}

fn print_json<T: serde::Serialize>(response: &ProviderResponse<T>) -> anyhow::Result<()> {
    print_pretty_json(response)
}

fn print_pretty_json<T: serde::Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn print_header<T>(response: &ProviderResponse<T>) {
    println!(
        "provider: {}  cache: {}  credits: {}  latency: {} ms",
        response.provider,
        response.cache_hit,
        response
            .credits
            .map(|credits| credits.to_string())
            .unwrap_or_else(|| "unknown".to_owned()),
        response.latency_ms
    );
}

fn display_title<'a>(title: &'a str, url: &'a str) -> &'a str {
    if title.is_empty() { url } else { title }
}

fn truncate(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_respects_char_boundaries() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello", 3), "hel…");
        assert_eq!(truncate("åäö", 2), "åä…");
    }
}
