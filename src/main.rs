use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell as CompletionShell};
use shadow_rs::shadow;
use std::io;
use tracing::info;
use tracing_subscriber::{filter::filter_fn, prelude::*};

// https://crates.io/crates/shadow-rs
shadow!(build);

mod cli;
mod cmd;
mod config;
mod errors;
mod install;
mod spinner;
mod system;
mod version;

use self::cli::RunCommand;
use self::cmd::add::Add;
use self::cmd::list::List;
use self::cmd::remove::Remove;
use self::cmd::update::Update;

#[derive(Debug, Clone, Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
#[clap(
    version=build::PKG_VERSION,
    long_version=build::CLAP_LONG_VERSION,
    about="Pull down releases from GitHub.",
    subcommand_required=true,
    arg_required_else_help=true,
)]
#[allow(clippy::upper_case_acronyms)]
pub struct CLI {
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,

    /// Clap subcommand to run.
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Commands {
    /// Add a package/release.
    Add(Add),
    /// Remove a package.
    #[clap(alias = "rm")]
    Remove(Remove),
    /// List installed packages.
    #[clap(alias = "ls")]
    List(List),
    /// Update packages to the latest version available from GitHub.
    #[clap(alias = "up")]
    Update(Update),
    /// Generate shell completions to stdout.
    Completions {
        #[clap(value_enum)]
        #[arg(short, long)]
        shell: CompletionShell,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    ctrlc::set_handler(|| {
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    let cli = CLI::parse();

    // Log from this crate only.
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(cli.verbose.log_level_filter().to_string()))
        .with(tracing_subscriber::fmt::layer().with_filter(filter_fn(|metadata| metadata.target().starts_with(env!("CARGO_PKG_NAME")))))
        .init();

    if let Some(env_api_token) = std::env::var_os("GITHUB_TOKEN") {
        info!("Initializing the GitHub client with token from environment");
        octocrab::initialise(octocrab::Octocrab::builder().personal_token(env_api_token.to_str().unwrap().into()).build()?);
    };

    match cli.command {
        Commands::Add(add) => add.run().await?,
        Commands::Remove(remove) => remove.run().await?,
        Commands::List(list) => list.run().await?,
        Commands::Update(update) => update.run().await?,

        Commands::Completions { shell } => generate(shell, &mut CLI::command(), "released", &mut io::stdout().lock()),
    };

    Ok(())
}
