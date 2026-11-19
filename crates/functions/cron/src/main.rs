use std::process::ExitCode;

use clap::{Parser, Subcommand};
use error::R;
use shared::env::Env;

mod cook_cakes;
mod destroy_stale_cakes;

#[derive(Parser)]
#[command(about = "Run scheduled cake jobs")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create cakes and their fruits.
    CookCakes,
    /// Delete cakes older than six hours and their fruits, then publish a notification.
    DestroyStaleCakes,
}

#[tokio::main]
async fn main() -> ExitCode {
    shared::tracing::install();
    shared::env::load();

    let args = Args::parse();

    match run(args).await {
        Err(err) => {
            tracing::error!(error = ?err, "Cron job failed");
            ExitCode::FAILURE
        }
        Ok(()) => ExitCode::SUCCESS,
    }
}

async fn run(args: Args) -> R<()> {
    let database_url = Env::DatabaseUrl.read()?;
    let db = database::connect_with_retry(&database_url, None, None).await?;

    match args.command {
        Command::CookCakes => cook_cakes::run(db).await,
        Command::DestroyStaleCakes => {
            let topic = Env::CleanupTopic.read()?;
            destroy_stale_cakes::run(db, &topic).await
        }
    }
}
