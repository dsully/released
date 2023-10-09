use anyhow::Result;
use skim::{
    prelude::{SkimItemReader, SkimOptionsBuilder},
    Skim,
};
use std::io::Cursor;
use tracing::{error, info};

use crate::{
    config::{Config, Package},
    errors::CommandError,
    install,
    system::System,
    version::parse_version,
};

pub struct Patterns {
    pub asset: Option<String>,
    pub file: Option<String>,
}

async fn repository_releases(owner: &'_ str, repository: &'_ str, pre_release: bool) -> Result<Vec<String>> {
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

pub async fn add(
    packages: &mut Config,
    name: &'_ str,
    system: &'_ System,
    patterns: Patterns,
    alias: Option<String>,
    show: bool,
    pre_release: bool,
) -> super::Result<()> {
    let split_name: Vec<&str> = name.split('@').collect();

    let org_repo = if split_name.len() > 1 { split_name[0] } else { name };
    let split_org_repo: Vec<&str> = org_repo.split('/').collect();

    let organization = split_org_repo[0];
    let repository = split_org_repo[1];
    let alias = alias.unwrap_or_else(|| repository.to_string());

    let asset_pattern = &patterns.asset.unwrap_or_default();
    let file_pattern = &patterns.file.unwrap_or_else(|| alias.clone());

    info!("Organization `{organization}`, Repo `{repository}`, Alias `{alias}`, Pattern `{asset_pattern}`, Filter `{file_pattern}`");

    let version: String = if split_name.len() > 1 {
        split_name[1].to_string()
    } else {
        let versions = repository_releases(organization, repository, pre_release).await?;

        if show || versions.len() > 1 {
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
                None => return Err(CommandError::ReleaseNotFound(name.to_string())),
            }
        }
    };

    let parsed_version = parse_version(&version);

    let package = Package::new(org_repo, &alias, asset_pattern, file_pattern);

    println!("Installing {} ...", &package.name);

    match install::install_release(packages, &package, system, Some(parsed_version), show).await {
        Ok(_) => println!("Installed {} successfully!", &package.name),
        Err(e) => error!("{:?}", e),
    }

    Ok(())
}
