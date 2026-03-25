mod client;
mod config;
mod profiles;
mod router;
mod session;
mod ui;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use crate::config::AppConfig;
use crate::ui::run_repl;

#[derive(Debug, Parser)]
#[command(name = "tinychat")]
#[command(about = "A lightweight terminal chat router for local model servers.")]
struct Cli {
    #[arg(long, default_value = "config/tinychat.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::load(&cli.config)
        .with_context(|| format!("failed to load config from {}", cli.config.display()))?;
    run_repl(config).await
}
