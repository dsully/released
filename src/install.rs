use anyhow::{Context, Result};
use compress_tools::*;
use octocrab::models::repos::{Asset, Release};
use regex::Regex;
use reqwest::Url;
use skim::prelude::*;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tracing::{debug, info};
use trauma::{
    download::Download,
    downloader::{DownloaderBuilder, ProgressBarOpts, StyleOptions},
    Error,
};
use walkdir::{DirEntry, WalkDir};

use crate::{
    config::{Config, InstalledPackage, Package},
    errors::CommandError,
    system::System,
    version::{parse_version, Version},
};

pub async fn download(url: &Url, directory: &PathBuf) -> Result<PathBuf, Error> {
    let style = ProgressBarOpts::new(
        Some(ProgressBarOpts::TEMPLATE_PIP.into()),
        Some(ProgressBarOpts::CHARS_LINE.into()),
        true,
        false,
    );

    let mut download = Download::try_from(url)?;
    let filename = directory.join(download.filename.clone());

    download.filename = filename.display().to_string();

    info!("Downloading file to {:?}", &filename);

    DownloaderBuilder::new()
        .directory(directory.into())
        .style_options(StyleOptions::new(style.clone(), style.clone()))
        .build()
        .download(&[download])
        .await;

    Ok(filename)
}

pub async fn release_for_repository(owner: &'_ str, repo: &'_ str, version: &'_ Version) -> Result<Release> {
    info!("Getting release: {} for {}/{}", version.as_tag(), owner, repo);

    let octo = octocrab::instance();

    if version == &Version::Latest {
        match octo.repos(owner, repo).releases().get_latest().await {
            Ok(latest_release) => Ok(latest_release),
            Err(e) => Err(e.into()),
        }
    } else {
        match octo.repos(owner, repo).releases().get_by_tag(&version.as_tag()).await {
            Ok(tagged_release) => Ok(tagged_release),
            Err(_) => match octo.repos(owner, repo).releases().get_by_tag(&format!("v{}", version.as_tag())).await {
                Ok(tagged_release) => Ok(tagged_release),
                Err(e) => Err(e.into()),
            },
        }
    }
}

pub async fn latest_release_tag(owner: &'_ str, repo: &'_ str) -> Option<Version> {
    match octocrab::instance().repos(owner, repo).releases().get_latest().await {
        Ok(tag) => Some(parse_version(&tag.tag_name)),
        Err(_) => None,
    }
}

pub fn platform_asset(release: &'_ Release, system: &'_ System, user_pattern: &'_ str, show: bool) -> Option<Asset> {
    //
    let regex = match user_pattern.is_empty() {
        false => Some(Regex::new(user_pattern).unwrap_or_else(|_| panic!("{} is not a valid Regular Expression", user_pattern))),
        true => None,
    };

    let platform_assets: Vec<Asset> = release
        .assets
        .iter()
        .filter_map(|asset| if asset.name.ends_with(".sha256") { None } else { Some(asset.clone()) })
        .filter_map(|asset| if asset.name.ends_with(".txt") { None } else { Some(asset.clone()) })
        .filter_map(|asset| {
            if let Some(r) = &regex {
                debug!("Matching '{}' against '{}'", r.as_str(), &asset.name);

                if r.is_match(&asset.name) {
                    Some(asset.clone())
                } else {
                    None
                }
            } else if system.is_match(&asset.name) {
                debug!("Asset info: {:?}", asset.name);
                Some(asset.clone())
            } else if show {
                Some(asset.clone())
            } else {
                None
            }
        })
        .collect();

    match &platform_assets.len() {
        2.. => {
            let item_reader =
                SkimItemReader::default().of_bufread(Cursor::new(platform_assets.iter().map(|a| a.name.to_string()).collect::<Vec<_>>().join("\n")));

            let selected_item: Vec<Asset> = Skim::run_with(
                &SkimOptionsBuilder::default()
                    .color(Some(crate::config::skim_colors()))
                    .height(Some("25%"))
                    .build()
                    .unwrap(),
                Some(item_reader),
            )
            .map(|items| {
                items
                    .selected_items
                    .iter()
                    .map(|item| platform_assets.clone().into_iter().find(|asset| asset.name == item.text()).unwrap())
                    .collect()
            })
            .unwrap();

            Some(selected_item.get(0).unwrap().to_owned())
        }
        1 => Some(platform_assets.get(0).unwrap().clone()),
        _ => None,
    }
}

fn find_binary(folder: &'_ Path, bin_name: &'_ str) -> Option<DirEntry> {
    WalkDir::new(folder)
        .into_iter()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name() == bin_name)
}

pub async fn install_release(config: &mut Config, package: &'_ Package, system: &'_ System, version: Option<Version>, show: bool) -> Result<()> {
    let split_org_repo: Vec<&str> = package.name.split('/').collect();

    let owner = split_org_repo[0];
    let repo = split_org_repo[1];

    let xdg_dir = xdg::BaseDirectories::with_prefix("released").context("Failed get XDG directory")?;

    let cache_path = xdg_dir.get_cache_home();
    let bin_path = crate::config::bin_path()?;

    let version = match version {
        Some(v) => v,
        None => match latest_release_tag(owner, repo).await {
            Some(rel) => rel,
            None => return Err(CommandError::ReleaseNotFound(package.name.to_owned()).into()),
        },
    };

    if let Some(installed) = config.installed.get(&package.alias) {
        //
        if installed.version == version.as_tag() {
            return Err(CommandError::NoUpdateNeeded.into());
        }
    }

    let release = release_for_repository(owner, repo, &version).await?;

    let asset = match platform_asset(&release, system, &package.asset_pattern, show) {
        Some(asset) => asset,
        None => {
            return Err(CommandError::AssetNotFound {
                package: package.name.to_string(),
                version,
                arch: system.architecture.clone(),
                os: system.os.clone(),
            }
            .into())
        }
    };

    match download(&asset.browser_download_url, &cache_path).await {
        Ok(asset_path) => {
            info!("Completed downloading {}", asset.browser_download_url);
            info!("Path: {}", asset_path.display());

            let source = fs::File::open(&asset_path).context("Unable to open downloaded file")?;

            uncompress_archive(&source, &cache_path, Ownership::Preserve).context("Unable to unarchive file")?;

            info!("Successfully extracted '{}'.", asset_path.display());

            let binary_file_name = if !&package.file_pattern.is_empty() {
                package.file_pattern.clone()
            } else {
                package.alias.clone()
            };

            match find_binary(&cache_path, &binary_file_name) {
                Some(bin_file) => {
                    let destination = bin_path.join(bin_file.file_name());

                    info!("Binary '{}'.", bin_file.path().display());
                    info!("Destination '{}'.", destination.display());

                    fs::rename(bin_file.path(), &destination).context(format!("Unable to move file to {}", bin_path.display()))?;
                    fs::remove_file(&asset_path).context("Unable to remove temporary file")?;

                    if !config.installed.contains_key(&package.alias) {
                        config.packages.insert(package.name.to_owned(), package.to_owned());
                    }

                    config.installed.insert(
                        package.alias.to_owned(),
                        InstalledPackage {
                            name: package.name.to_owned(),
                            version: version.as_tag().to_owned(),
                            path: destination.to_owned(),
                        },
                    );

                    config.save()?;

                    Ok(())
                }
                _ => Err(CommandError::UnableToFindBinaryError { binary_file_name }.into()),
            }
        }
        Err(_) => Err(CommandError::AssetDownloadError {
            asset_uri: asset.browser_download_url,
            asset_name: asset.name,
        }
        .into()),
    }
}
