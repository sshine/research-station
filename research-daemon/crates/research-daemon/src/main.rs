use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use research_core::Config;
use research_service::{Worker, WorkerOptions, build_provider};
use research_store::JobStore;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "researchd",
    version,
    about = "Research queue worker for research-station"
)]
struct Cli {
    /// Path to researchd.toml. Defaults to $RESEARCHD_CONFIG, the repository
    /// config, then the XDG config directory.
    #[arg(long, env = "RESEARCHD_CONFIG")]
    config: Option<PathBuf>,

    /// Use the deterministic fake provider instead of Tavily.
    #[arg(long)]
    fake: bool,

    /// Drain the current queue once, then exit.
    #[arg(long)]
    once: bool,

    /// Lease owner name shown on jobs.
    #[arg(long, default_value = "researchd")]
    owner: String,

    /// Idle poll interval in seconds. Defaults to the configured value.
    #[arg(long)]
    poll_seconds: Option<u64>,
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
    let config = Config::load(config_path.as_deref()).context("loading configuration")?;
    let store = JobStore::open(config.database_path())
        .await
        .context("opening job database")?;

    let mut options = WorkerOptions::from_config(&config, cli.owner);
    if let Some(seconds) = cli.poll_seconds {
        options.poll_interval = Duration::from_secs(seconds.max(1));
    }

    let provider = build_provider(&config, cli.fake)
        .await
        .context("creating provider")?;
    let worker = Worker::new(store, provider, options);
    worker.reap_expired().await?;

    if cli.once {
        let processed = worker.run_until_idle().await?;
        println!("processed {processed} job(s)");
        return Ok(());
    }

    println!(
        "researchd: owner={} provider={} database={}",
        worker.options().owner,
        if cli.fake { "fake" } else { "tavily" },
        config.database_path().display()
    );
    worker
        .run_forever(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    println!("researchd stopped");
    Ok(())
}
