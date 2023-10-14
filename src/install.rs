use anyhow::{Context, Result};
use compress_tools::*;
use futures::stream::StreamExt;
use octocrab::models::repos::{Asset, Release};
use regex::Regex;
use reqwest::Url;
use skim::prelude::*;
use std::fs;
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};
use walkdir::{DirEntry, WalkDir};

use crate::{
    config::{Config, InstalledPackage, Package},
    errors::CommandError,
    spinner::spinner,
    system::System,
    version::{parse_version, Version},
};

pub async fn download(url: &Url, directory: &PathBuf) -> Result<PathBuf> {
    // TODO: Add better error handling.

    let filename = url
        .path_segments()
        .ok_or_else(|| CommandError::InvalidUrl(url.to_string()))?
        .rev()
        .find(|segment| !segment.is_empty())
        .unwrap();

    let destination = directory.join(filename);

    debug!("Creating destination directory {:?}", directory);

    fs::create_dir_all(directory)?;

    debug!("Creating destination file {:?}", &destination);

    let mut file = tokio::fs::File::create(&destination).await?;

    let s = spinner();

    s.set_message(format!("Downloading {} ...", &filename));

    let mut stream = reqwest::get(url.clone()).await?.error_for_status()?.bytes_stream();

    while let Some(item) = stream.next().await {
        file.write_all_buf(&mut item.context("Unable to retrieve next chunk from download stream..")?)
            .await?;
    }

    s.finish_with_message(format!("Downloaded {}", &filename));

    Ok(destination)
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

pub fn platform_asset(release: &'_ Release, system: &'_ System, user_pattern: &'_ str, _show: bool) -> Option<Asset> {
    //
    let regex = match user_pattern.is_empty() {
        false => Some(Regex::new(user_pattern).unwrap_or_else(|_| panic!("{} is not a valid Regular Expression", user_pattern))),
        true => None,
    };

    let mut platform_assets: Vec<Asset> = release
        .assets
        .iter()
        .filter_map(|asset| if asset.name.ends_with(".sha256") { None } else { Some(asset.clone()) })
        .filter_map(|asset| if asset.name.ends_with(".txt") { None } else { Some(asset.clone()) })
        .collect();

    platform_assets = platform_assets
        .iter()
        .filter_map(|asset| {
            if let Some(r) = &regex {
                if r.is_match(&asset.name) {
                    debug!("User pattern matched '{}' against '{}'", r.as_str(), &asset.name);
                    Some(asset.clone())
                } else {
                    None
                }
            } else if system.is_match(&asset.name) {
                debug!("System and OS match '{}'", &asset.name);
                Some(asset.clone())
            } else {
                None
            }
        })
        .collect();

    if platform_assets.is_empty() {
        platform_assets = release
            .assets
            .iter()
            .filter_map(|asset| {
                if system.is_os_match(&asset.name) {
                    debug!("OS match '{}'", &asset.name);
                    Some(asset.clone())
                } else {
                    None
                }
            })
            .collect();
    }

    if platform_assets.is_empty() {
        platform_assets = release.assets.clone();
    }

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

    let temp_path = tempdir().context("Unable to create temporary directory")?.into_path();

    match download(&asset.browser_download_url, &temp_path).await {
        Ok(asset_path) => {
            info!("Completed downloading {}", asset.browser_download_url);
            info!("Path: {}", asset_path.display());

            let mut is_standalone = false;

            match infer::get_from_path(&asset_path) {
                Ok(Some(ft)) if ft.matcher_type() == infer::MatcherType::Archive => {
                    //
                    let source = fs::File::open(&asset_path).context("Unable to open downloaded file")?;

                    uncompress_archive(&source, &cache_path, Ownership::Preserve).context("Unable to unarchive file")?;

                    info!("Successfully extracted '{}'.", asset_path.display());
                }
                Ok(Some(ft)) if ft.matcher_type() == infer::MatcherType::App => is_standalone = true,
                Ok(Some(ft)) if ft.mime_type() == "text/x-shellscript" => is_standalone = true,
                Ok(Some(ft)) => {
                    return Err(CommandError::InvalidFileTypeError {
                        path: asset_path,
                        ft: ft.mime_type().to_string(),
                    }
                    .into())
                }
                _ => {
                    return Err(CommandError::InvalidFileTypeError {
                        path: asset_path,
                        ft: String::from("Unknown"),
                    }
                    .into())
                }
            }

            let binary_file_name = if is_standalone {
                asset_path.to_string_lossy().to_string()
            } else if package.file_pattern.is_empty() {
                package.alias.clone()
            } else {
                package.file_pattern.clone()
            };

            let source = if is_standalone {
                asset_path
            } else {
                match find_binary(&cache_path, &binary_file_name) {
                    Some(bin_file) => bin_file.into_path(),
                    None => return Err(CommandError::UnableToFindBinaryError { binary_file_name }.into()),
                }
            };

            let destination = if is_standalone {
                bin_path.join(&package.alias)
            } else {
                bin_path.join(source.file_name().unwrap())
            };

            info!("Binary '{}'.", source.display());
            info!("Renaming to '{}' and setting executable.", destination.display());

            fs::rename(&source, &destination).context(format!("Unable to move file to {}", bin_path.display()))?;
            fs::set_permissions(&destination, fs::Permissions::from_mode(0o755))?;

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
        Err(_) => Err(CommandError::AssetDownloadError {
            asset_uri: asset.browser_download_url,
            asset_name: asset.name,
        }
        .into()),
    }
}
