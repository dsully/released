use std::fmt::Display;
use tracing::info;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Version {
    /// The latest version of a tool as determined by the tool managers
    Latest,
    /// SemVer should follow the patterns outlined at [SemVer.org](https://semver.org/)
    SemVer(semver::Version),
    /// LTS is not available for all tool managers, this will be a version that isn't the latest but is supported longer than a typical release.
    Lts,
    /// Stable often times is synonymous with Latest however this will ensure that the version is considered stable before installing.
    Stable,
    /// Similar to Latest except will use pre-releases if there are any available for the tool
    PreRelease,
    ///
    Simple(String),
}

impl Default for Version {
    fn default() -> Self {
        Self::Latest
    }
}

// Write an Into for Version

impl Version {
    pub fn as_tag(&self) -> String {
        match self {
            Version::SemVer(v) => v.to_string(),
            Version::Simple(s) => s.to_string(),
            Version::Latest => "latest".to_string(),
            Version::Lts => "lts".to_string(),
            Version::Stable => "stable".to_string(),
            Version::PreRelease => "pre-release".to_string(),
        }
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_tag())
    }
}

pub fn parse_version(provided_version: &'_ str) -> Version {
    match provided_version.to_lowercase().as_str() {
        "latest" => Version::Latest,
        "stable" => Version::Stable,
        "lts" => Version::Lts,
        "prerelease" | "pre-release" => Version::PreRelease,
        _ => {
            let provided_version = provided_version.trim_start_matches('v');
            info!("Parsing version: {}", provided_version);

            if let Ok(parsed_semver) = semver::Version::parse(provided_version) {
                Version::SemVer(parsed_semver)
            } else {
                Version::Simple(provided_version.to_string())
            }
        }
    }
}
