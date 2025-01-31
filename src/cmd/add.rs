use std::io::Cursor;

use clap::Args;
use git_url_parse::GitUrl;
use skim::{
    prelude::{SkimItemReader, SkimOptionsBuilder},
    Skim,
};
use tracing::{error, info};

use crate::{
    cli::{Result, RunCommand},
    config::{Config, Package},
    errors::CommandError,
    install,
    spinner::spinner,
    system::System,
    version,
};

#[derive(Debug, Clone, Args)]
pub struct Add {
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
}

pub struct Patterns {
    pub asset: Option<String>,
    pub file: Option<String>,
}

async fn repository_releases(owner: &'_ str, repository: &'_ str, pre_release: bool) -> anyhow::Result<Vec<String>> {
    Ok(octocrab::instance()
        .repos(owner, repository)
        .releases()
        .list()
        .per_page(100)
        .send()
        .await?
        .items
        .iter()
        .filter_map(|release| {
            if pre_release && release.prerelease {
                None
            } else {
                Some(release.tag_name.clone())
            }
        })
        .collect())
}

impl RunCommand for Add {
    //
    async fn run(self) -> Result<()> {
        let mut packages = Config::load()?;
        let system = System::default();

        let split_name: Vec<&str> = self.name.split('@').collect();

        let urlish = if split_name.len() > 1 { split_name[0].to_string() } else { self.name.clone() };

        let urlish = if urlish.contains("github.com") {
            urlish
        } else {
            format!("https://github.com/{urlish}")
        };

        let url = GitUrl::parse(&urlish).expect("Couldn't parse as a GitHub URL!");

        let organization = url.owner.expect("Couldn't find an organization!");
        let repository = url.name;
        let alias = self.alias.unwrap_or_else(|| repository.clone());

        let patterns = Patterns {
            asset: self.asset_pattern,
            file: self.file_filter,
        };

        let asset_pattern = &patterns.asset.unwrap_or_default();
        let file_pattern = &patterns.file.unwrap_or_else(|| alias.clone());

        info!("Organization `{organization}`, Repo `{repository}`, Alias `{alias}`, Pattern `{asset_pattern}`, Filter `{file_pattern}`");

        let version: String = if split_name.len() > 1 {
            split_name[1].to_string()
        } else {
            let versions = repository_releases(&organization, &repository, self.pre_release).await?;

            if self.show {
                let reader = SkimItemReader::default().of_bufread(Cursor::new(versions.join("\n")));

                Skim::run_with(
                    &SkimOptionsBuilder::default()
                        .color(Some(crate::config::skim_colors().to_string()))
                        .height("50%".to_string())
                        .multi(true)
                        .reverse(true)
                        .build()
                        .expect("Unable to build SkimOptionsBuilder"),
                    Some(reader),
                )
                .map(|items| items.selected_items.iter().map(|item| item.text().to_string()).collect())
                .unwrap_or_default()
            } else {
                match versions.first() {
                    Some(version) => version.into(),
                    None => return Err(CommandError::ReleaseNotFound(self.name.clone())),
                }
            }
        };

        let parsed_version = version::parse(&version);

        let package = Package::new(&format!("{organization}/{repository}"), &alias, asset_pattern, file_pattern);

        let s = spinner();

        s.set_message(format!("⊙ Installing {} ...", &package.name));

        match install::install_release(&mut packages, &package, &system, Some(parsed_version), self.show).await {
            Ok(()) => s.finish_with_message(format!("Installed {} successfully!", &package.name)),
            Err(e) => {
                s.finish();
                error!("{:?}", e);
            }
        }

        Ok(())
    }
}
