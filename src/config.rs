use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use toml;
use tracing::debug;

use crate::errors::ConfigError;

static SKIM_COLORS: OnceLock<String> = OnceLock::new();

pub fn skim_colors() -> &'static str {
    SKIM_COLORS.get_or_init(|| {
        vec![
            "bg+:#3B4252",
            "bg:#2E3440",
            "spinner:#81A1C1",
            "hl:#616E88",
            "fg:#D8DEE9",
            "header:#616E88",
            "info:#81A1C1",
            "pointer:#81A1C1",
            "marker:#81A1C1",
            "fg+:#81A1C1",
            "prompt:#81A1C1",
            "hl+:#81A1C1",
        ]
        .join(",")
    })
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(flatten)]
    pub packages: BTreeMap<String, Package>,

    #[serde(skip)]
    pub installed: BTreeMap<String, InstalledPackage>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "snake_case", default)]
pub struct Package {
    pub name: String,
    pub alias: String,
    pub asset_pattern: String,
    pub file_pattern: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
#[serde(rename_all = "snake_case", default)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
}

pub fn bin_path() -> Result<PathBuf> {
    #[allow(deprecated)]
    let home_dir = std::env::home_dir().context("Get HOME directory")?;

    let bin_path = home_dir.join(".local/bin/");

    if !bin_path.exists() {
        fs::create_dir_all(&bin_path).context("Creating `~/.local/bin`")?;
    }

    Ok(bin_path.to_path_buf())
}

pub fn config_path() -> Result<PathBuf> {
    let xdg_dir = xdg::BaseDirectories::with_prefix("released").context("Failed get config directory")?;

    match xdg_dir.place_config_file("config.toml") {
        Ok(path) => Ok(path),
        Err(e) => {
            return Err(ConfigError::FailedToCreateDirectory {
                path: xdg_dir.get_config_home(),
                source: e,
            }
            .into())
        }
    }
}

pub fn state_path() -> Result<PathBuf> {
    let xdg_dir = xdg::BaseDirectories::with_prefix("released").context("Failed get config directory")?;

    match xdg_dir.place_state_file("installed.json") {
        Ok(path) => Ok(path),
        Err(e) => {
            return Err(ConfigError::FailedToCreateDirectory {
                path: xdg_dir.get_state_home(),
                source: e,
            }
            .into())
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_file = config_path()?;
        let state_file = state_path()?;

        let mut config: Config = match config_file.exists() {
            true => {
                debug!("Reading config file from {:?}", &config_file);

                let builder = config::Config::builder()
                    .add_source(config::File::from(config_file.as_ref()))
                    .build()
                    .context("Failed to create Config builder.")?;

                match builder.try_deserialize() {
                    Ok(c) => c,
                    Err(e) => {
                        return Err(ConfigError::DeserializationError {
                            file_path: config_file,
                            format: "TOML".to_string(),
                            msg: e.to_string(),
                        }
                        .into());
                    }
                }
            }
            false => Config::default(),
        };

        config.installed = match fs::read_to_string(&state_file) {
            Ok(s) => match serde_json::from_str(&s) {
                Ok(c) => c,
                Err(e) => {
                    return Err(ConfigError::DeserializationError {
                        file_path: state_file,
                        format: "JSON".to_string(),
                        msg: e.to_string(),
                    }
                    .into())
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => {
                return Err(ConfigError::FileReadError {
                    file_path: state_file,
                    source: e,
                }
                .into());
            }
        };

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_file = self::config_path()?;
        let state_file = self::state_path()?;

        debug!("Writing config file to {:?}", &config_file);

        fs::write(&config_file, toml::to_string(&self.packages).context("Serializing config into TOML format")?)
            .context(format!("Writing config file: {}", config_file.display()))?;

        debug!("Writing installed file to {:?}", &state_file);

        let state = to_string_pretty(&self.installed).context(format!("Failed to serialize state to JSON."))?;

        fs::write(&state_file, state).context(format!("Failed to write state file!"))?;

        debug!("Wrote installed state to file {:?}", &state_file);

        Ok(())
    }
}

impl Package {
    pub fn new(name: &'_ str, alias: &'_ str, asset_pattern: &'_ str, file_pattern: &'_ str) -> Self {
        Self {
            name: name.to_string(),
            alias: alias.to_string(),
            asset_pattern: asset_pattern.to_string(),
            file_pattern: file_pattern.to_string(),
        }
    }
}
