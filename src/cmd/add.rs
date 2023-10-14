use async_trait::async_trait;
use clap::Args;
use skim::{
    prelude::{SkimItemReader, SkimOptionsBuilder},
    Skim,
};
use std::io::Cursor;
use tracing::{error, info};

use crate::{
    cli::{Result, RunCommand},
    config::{Config, Package},
    errors::CommandError,
    install,
    system::System,
    version::parse_version,
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
        .filter_map(|release| match pre_release {
            true => Some(release.tag_name.to_string()),
            false => match release.prerelease {
                true => None,
                false => Some(release.tag_name.to_string()),
            },
        })
        .collect())
}

#[async_trait]
impl RunCommand for Add {
    async fn run(self) -> Result<()> {
        let mut packages = Config::load()?;
        let system = System::default();

        let split_name: Vec<&str> = self.name.split('@').collect();

        let org_repo = if split_name.len() > 1 { split_name[0] } else { &self.name };
        let split_org_repo: Vec<&str> = org_repo.split('/').collect();

        let organization = split_org_repo[0];
        let repository = split_org_repo[1];
        let alias = self.alias.unwrap_or_else(|| repository.to_string());

        let patterns = Patterns {
            asset: self.asset_pattern.to_owned(),
            file: self.file_filter.to_owned(),
        };

        let asset_pattern = &patterns.asset.unwrap_or_default();
        let file_pattern = &patterns.file.unwrap_or_else(|| alias.clone());

        info!("Organization `{organization}`, Repo `{repository}`, Alias `{alias}`, Pattern `{asset_pattern}`, Filter `{file_pattern}`");

        let version: String = if split_name.len() > 1 {
            split_name[1].to_string()
        } else {
            let versions = repository_releases(organization, repository, self.pre_release).await?;

            if self.show {
                let reader = SkimItemReader::default().of_bufread(Cursor::new(versions.join("\n")));

                Skim::run_with(
                    &SkimOptionsBuilder::default()
                        .color(Some(crate::config::skim_colors()))
                        .height(Some("50%"))
                        .multi(true)
                        .reverse(true)
                        .build()
                        .unwrap(),
                    Some(reader),
                )
                .map(|items| items.selected_items.iter().map(|item| item.text().to_string()).collect())
                .unwrap_or_default()
            } else {
                match versions.get(0) {
                    Some(version) => version.into(),
                    None => return Err(CommandError::ReleaseNotFound(self.name.to_string())),
                }
            }
        };

        let parsed_version = parse_version(&version);

        let package = Package::new(org_repo, &alias, asset_pattern, file_pattern);

        println!("Installing {} ...", &package.name);

        match install::install_release(&mut packages, &package, &system, Some(parsed_version), self.show).await {
            Ok(_) => println!("Installed {} successfully!", &package.name),
            Err(e) => error!("{:?}", e),
        }

        Ok(())
    }
}
