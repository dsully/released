use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell as CompletionShell};
use shadow_rs::shadow;
use std::io;
use tracing::info;
use tracing_subscriber::{filter::filter_fn, prelude::*};

// https://crates.io/crates/shadow-rs
shadow!(build);

mod cmd;
mod config;
mod errors;
mod install;
mod system;
mod version;

use self::config::Config;
use self::system::System;

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
    Add {
        /// Name of the package to install.
        ///
        /// To install a specific version use name@version, for example: `cli/cli@v2.4.0`
        name: String,
        /// Alias to use instead of the repository name.
        ///
        /// This is how you will call the package on the command line.
        #[arg(short = 'A', long)]
        alias: Option<String>,
        /// Pattern used to determine which file from the release to download.
        ///
        /// This can be used to override the autodetect mechanism to determine which assets to download.
        #[arg(short, long)]
        asset_pattern: Option<String>,
        /// Filter used to find the executable.
        #[arg(short, long)]
        file_filter: Option<String>,
        /// Allow install of pre-release versions of the package.
        ///
        /// When `show` is provided this includes pre-release versions in the list,
        /// When it is not the most recent version is selected, that could be a pre-release or
        /// official release depending on release date.
        #[arg(short = 'P', long)]
        pre_release: bool,
        /// Show available versions
        ///
        /// Shows a list of versions available to install.
        #[arg(short = 'S', long)]
        show: bool,
    },
    /// Remove a package.
    Remove {
        /// Name of the package to remove.
        name: String,
    },
    /// List installed packages.
    List,
    /// Update packages to the latest version available from GitHub.
    Update {
        /// Which package to update, when omitted all packages will be updated.
        name: Option<String>,
    },

    /// Generate shell completions to stdout.
    Completions {
        #[clap(value_enum)]
        #[arg(short, long)]
        shell: CompletionShell,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
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

    let system = System::default();

    let mut packages = Config::load()?;

    match cli.command {
        Commands::Add {
            name,
            alias,
            asset_pattern,
            file_filter,
            pre_release,
            show,
        } => {
            cmd::add(
                &mut packages,
                &name,
                &system,
                cmd::Patterns {
                    asset: asset_pattern.to_owned(),
                    file: file_filter.to_owned(),
                },
                alias.to_owned(),
                show,
                pre_release,
            )
            .await?
        }
        Commands::Remove { name } => cmd::remove(&mut packages, &name).await?,
        Commands::List => cmd::list(&packages).await?,
        Commands::Update { name } => cmd::update(&mut packages, &system, name).await?,

        Commands::Completions { shell } => generate(shell, &mut CLI::command(), "released", &mut io::stdout().lock()),
    };

    Ok(())
}
