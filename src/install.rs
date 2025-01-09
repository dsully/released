use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use decompress::{decompress, ExtractOptsBuilder};
use futures::stream::StreamExt;
use itertools::Itertools;
use octocrab::models::repos::{Asset, Release};
use regex::Regex;
use reqwest::Url;
use skim::prelude::*;
use strfmt::strfmt;
use tempfile::tempdir;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};
use walkdir::{DirEntry, WalkDir};

use crate::{
    config::{Config, InstalledPackage, Package},
    errors::CommandError,
    system::System,
    version::{self, Version},
};

pub async fn download(url: &Url, directory: &'_ Path) -> Result<PathBuf> {
    // TODO: Add better error handling.

    let filename = url
        .path_segments()
        .ok_or_else(|| CommandError::InvalidUrl(url.to_string()))?
        .rev()
        .find(|segment| !segment.is_empty())
        .expect("No non-empty segments found in URL path");

    let destination = directory.join(filename);

    debug!("Creating destination directory {}", directory.display());

    fs::create_dir_all(directory)?;

    debug!("Creating destination file {}", &destination.display());

    let mut file = tokio::fs::File::create(&destination).await?;

    debug!("Downloading {filename} ...");

    let mut stream = reqwest::get(url.clone()).await?.error_for_status()?.bytes_stream();

    while let Some(item) = stream.next().await {
        file.write_all_buf(&mut item.context("Unable to retrieve next chunk from download stream..")?)
            .await?;
    }

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
    octocrab::instance()
        .repos(owner, repo)
        .releases()
        .get_latest()
        .await
        .ok()
        .map(|tag| version::parse(&tag.tag_name))
}

pub fn platform_asset(release: &'_ Release, system: &'_ System, user_pattern: &'_ str, _show: bool) -> Option<Asset> {
    //
    // First pass, remove all assets that are not for the current platform.
    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    let mut platform_assets: Vec<Asset> = release
        .assets
        .iter()
        .filter(|asset| !asset.name.ends_with(".sha256") && !asset.name.ends_with(".txt") && !asset.name.ends_with(".sig"))
        .cloned()
        .collect();

    // Only one asset, such as diff-so-fancy?
    if platform_assets.len() == 1 {
        debug!("Only one asset, returning: {}", platform_assets[0].name);

        return Some(platform_assets[0].clone());
    }

    // Second pass - use the user provided pattern to match against the asset name if provided.
    // If the regex contains the OS or architecture placeholders, insert them into the pattern.
    //
    // Otherwise, match against the OS of the current system.
    platform_assets = if user_pattern.is_empty() {
        platform_assets.iter().filter(|asset| system.is_os_match(&asset.name)).cloned().collect()
    } else {
        let s = HashMap::from([
            ("os".to_string(), system.os.get_match_regex().to_string()),
            ("arch".to_string(), system.architecture.get_match_regex().to_string()),
        ]);

        let pattern = strfmt(user_pattern, &s).expect("Invalid pattern");

        debug!("Matching against pattern: {}", pattern);

        let r = Regex::new(&pattern).unwrap_or_else(|_| panic!("{} is not a valid Regular Expression", &pattern));

        platform_assets.iter().filter(|asset| r.is_match(&asset.name)).cloned().collect()
    };

    // TODO: Handle macOS / Universal case.

    if platform_assets.len() == 1 {
        return Some(platform_assets[0].clone());
    }

    // Pass through the assets again, this time matching against the architecture.
    platform_assets.retain(|asset| system.is_arch_match(&asset.name));

    if platform_assets.is_empty() {
        platform_assets.clone_from(&release.assets);
    }

    match &platform_assets.len() {
        2.. => {
            let assets_list = platform_assets.iter().map(|a| a.name.clone()).collect_vec().join("\n");

            let reader = SkimItemReader::default().of_bufread(Cursor::new(assets_list));

            let selected_item: Vec<Asset> = Skim::run_with(
                &SkimOptionsBuilder::default()
                    .color(Some(crate::config::skim_colors().to_string()))
                    .height("25%".to_string())
                    .build()
                    .expect("Unable to build SkimOptionsBuilder"),
                Some(reader),
            )
            .map(|items| {
                items
                    .selected_items
                    .iter()
                    .map(|item| {
                        platform_assets
                            .clone()
                            .into_iter()
                            .find(|asset| asset.name == item.text())
                            .expect("Unable to find selected item")
                    })
                    .collect()
            })
            .expect("Unable to run Skim");

            Some(selected_item[0].clone())
        }
        1 => Some(platform_assets[0].clone()),
        _ => None,
    }
}

fn find_binary(folder: &'_ Path, bin_name: &'_ str) -> Option<DirEntry> {
    WalkDir::new(folder)
        .into_iter()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name() == bin_name)
}

#[allow(clippy::module_name_repetitions)]
pub async fn install_release(config: &mut Config, package: &'_ Package, system: &'_ System, version: Option<Version>, show: bool) -> Result<()> {
    let (owner, repo) = package.name.split_once('/').expect("Invalid package name");

    let bin_path = crate::config::bin_path()?;

    let version = match version {
        Some(v) => v,
        None => match latest_release_tag(owner, repo).await {
            Some(rel) => rel,
            None => return Err(CommandError::ReleaseNotFound(package.name.clone()).into()),
        },
    };

    if let Some(installed) = config.installed.get(&package.alias) {
        //
        if installed.version == version.as_tag() {
            return Err(CommandError::NoUpdateNeeded.into());
        }
    }

    let release = release_for_repository(owner, repo, &version).await?;

    let Some(asset) = platform_asset(&release, system, &package.asset_pattern, show) else {
        return Err(CommandError::AssetNotFound {
            package: package.name.clone(),
            version,
            arch: system.architecture.clone(),
            os: system.os.clone(),
        }
        .into());
    };

    let temp_dir = tempdir().context("Unable to create temporary directory")?;
    let temp_path = temp_dir.path();

    if let Ok(asset_path) = download(&asset.browser_download_url, temp_path).await {
        info!("Completed downloading {}", asset.browser_download_url);
        info!("Path: {asset_path:?}");

        let mut is_standalone = false;

        match infer::get_from_path(&asset_path) {
            Ok(Some(ft)) if ft.matcher_type() == infer::MatcherType::Archive => {
                decompress(&asset_path, &temp_path.into(), &ExtractOptsBuilder::default().build()?).context("Unable to unarchive file")?;

                info!("Successfully extracted '{asset_path:?}'.");
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
            match find_binary(temp_path, &binary_file_name) {
                Some(bin_file) => bin_file.into_path(),
                None => return Err(CommandError::UnableToFindBinaryError { binary_file_name }.into()),
            }
        };

        let destination = if is_standalone {
            bin_path.join(&package.alias)
        } else {
            bin_path.join(source.file_name().expect("Unable to get file name"))
        };

        info!("Binary '{source:?}'.");
        info!("Renaming to '{destination:?}' and setting executable.");

        fs::copy(&source, &destination).context(format!("Unable to copy {source:?} to {destination:?}"))?;
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o755))?;

        if !config.installed.contains_key(&package.alias) {
            config.packages.insert(package.name.clone(), package.clone());
        }

        config.installed.insert(
            package.alias.clone(),
            InstalledPackage {
                name: package.name.clone(),
                version: version.as_tag().clone(),
                path: destination.clone(),
            },
        );

        config.save()?;

        drop(temp_dir);

        Ok(())
    } else {
        drop(temp_dir);

        Err(CommandError::AssetDownloadError {
            asset_uri: asset.browser_download_url,
            asset_name: asset.name,
        }
        .into())
    }
}
