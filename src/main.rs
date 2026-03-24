mod cli;
mod drive;
mod fileops;
mod models;
mod normalize;
mod pipeline;
mod rules;
mod supabase;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Backup(args) => drive::run_backup(args).await?,
        Commands::Officialize(args) => supabase::run_officialize(args).await?,
    }
    Ok(())
}
