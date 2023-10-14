use crate::{
    system::{OperatingSystem, PlatformArchitecture},
    version::Version,
};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("Failed to delete file '{file_name}'")]
    FileDelete { file_name: PathBuf },

    #[error("Package '{name}' not found in config.")]
    PackageNotFound { name: String },

    #[error("Already up to date.")]
    NoUpdateNeeded,

    #[error("The config list does not contain any packages!")]
    EmptyPackages,

    #[error("Unable to find release for {0}")]
    ReleaseNotFound(String),

    #[error("Unable to find asset for {package}@{version} for OS: {os}; Arch: {arch}")]
    AssetNotFound {
        package: String,
        version: Version,
        arch: PlatformArchitecture,
        os: OperatingSystem,
    },

    #[error("Downloaded file isn't an archive or executable: '{path}': {ft}")]
    InvalidFileTypeError { path: PathBuf, ft: String },

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Unable to find binary '{binary_file_name}' in ~/.local/bin/")]
    UnableToFindBinaryError { binary_file_name: String },

    #[error("Failed to download file '{asset_name}' from '{asset_uri}'")]
    AssetDownloadError { asset_uri: reqwest::Url, asset_name: String },

    #[error("Error with the GitHub API {0}")]
    GitHub(#[from] octocrab::Error),

    #[error(transparent)]
    AnyHow(#[from] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to create the directory '{path}'. {source}")]
    FailedToCreateDirectory { path: PathBuf, source: std::io::Error },

    #[error("Unable to read file '{file_path}'. {source:?}")]
    FileReadError { file_path: PathBuf, source: std::io::Error },

    #[error("Failed to deserialize file: {file_path}, using {format}. {msg}")]
    DeserializationError {
        file_path: PathBuf,
        format: String,
        msg: String,
    },

    #[error(transparent)]
    AnyHow(#[from] anyhow::Error),
}
