use std::env::consts::{ARCH, OS};
use std::fmt::Display;

use regex::Regex;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct System {
    pub architecture: PlatformArchitecture,
    pub os: OperatingSystem,
}

impl Default for System {
    fn default() -> Self {
        Self {
            architecture: match ARCH {
                "x86_64" => PlatformArchitecture::Amd64,
                "aarch64" => PlatformArchitecture::Arm64,
                _ => panic!("Running on a unknown system architecture!"),
            },
            os: match OS {
                "linux" => OperatingSystem::Linux,
                "macos" => OperatingSystem::Darwin,
                _ => panic!("Running on a unknown OS!"),
            },
        }
    }
}

impl System {
    #[allow(dead_code)]
    pub fn is_match(&self, s: &'_ str) -> bool {
        self.is_os_match(s) && (self.is_arch_match(s) || Self::is_universal_match(s))
    }

    pub fn is_os_match(&self, s: &'_ str) -> bool {
        let os_regex = self.os.get_match_regex();

        debug!("OS Regex[{}], trying to match {}", os_regex.to_string(), s);

        os_regex.is_match(s)
    }

    pub fn is_arch_match(&self, s: &'_ str) -> bool {
        let arch_regex = self.architecture.get_match_regex();

        debug!("Architecture Regex[{}], trying to match {}", arch_regex.to_string(), s);

        arch_regex.is_match(s)
    }

    #[allow(dead_code)]
    pub fn is_universal_match(s: &'_ str) -> bool {
        debug!("macOS Universal trying to match: {}", s);

        Regex::new(r"(?i).*universal").expect("Unable to create regex for macOS Universal").is_match(s)
    }
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum OperatingSystem {
    Linux,
    Darwin,
}

impl Display for OperatingSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            OperatingSystem::Linux => write!(f, "Linux"),
            OperatingSystem::Darwin => write!(f, "macOS"),
        }
    }
}

impl OperatingSystem {
    pub fn get_match_regex(&self) -> Regex {
        match self {
            Self::Linux => Regex::new(r"(linux|unknown-linux-gnu)").expect("Unable to create regex for Linux"),
            Self::Darwin => Regex::new(r"(mac|macos|darwin)").expect("Unable to create regex for macOS"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum PlatformArchitecture {
    Amd64,
    Arm64,
}

impl Display for PlatformArchitecture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            PlatformArchitecture::Amd64 => write!(f, "x86_64"),
            PlatformArchitecture::Arm64 => write!(f, "arm64"),
        }
    }
}

impl PlatformArchitecture {
    pub fn get_match_regex(&self) -> Regex {
        match self {
            Self::Amd64 => Regex::new(r"(amd64|x86_64)").expect("Unable to create regex for amd64"),
            Self::Arm64 => Regex::new(r"(arm64|aarch64)").expect("Unable to create regex for arm64"),
        }
    }
}
