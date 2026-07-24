//! Layered CLI configuration schema.
//!
//! [`CliConfig`] is the single typed schema used for configuration discovery
//! and merging. It captures global CLI settings plus per-subcommand defaults
//! under the `cmds` namespace.

use clap::ValueEnum;
use ortho_config::{OrthoConfig, OrthoResult, PostMergeContext, PostMergeHook};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use super::validation_error;
use crate::host_pattern::HostPattern;

/// Required non-interactive execution setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NoInput(bool);

impl NoInput {
    /// Return whether interactive input is disabled.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        self.0
    }
}

impl Default for NoInput {
    fn default() -> Self {
        Self(true)
    }
}

/// Colour-output policy accepted by layered configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ColourPolicy {
    /// Follow the host environment.
    #[default]
    Auto,
    /// Force colour output on when available.
    Always,
    /// Force colour output off.
    Never,
}

impl fmt::Display for ColourPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Always => write!(f, "always"),
            Self::Never => write!(f, "never"),
        }
    }
}

impl FromStr for ColourPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <Self as ValueEnum>::from_str(s, true).map_err(|_| format!("invalid color policy '{s}'"))
    }
}

/// Progress rendering policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ProgressPolicy {
    /// Follow Netsuke's default progress behaviour.
    #[default]
    Auto,
    /// Force progress rendering on.
    Always,
    /// Disable progress rendering.
    Never,
}

impl fmt::Display for ProgressPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Always => write!(f, "always"),
            Self::Never => write!(f, "never"),
        }
    }
}

impl FromStr for ProgressPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <Self as ValueEnum>::from_str(s, true).map_err(|_| format!("invalid progress policy '{s}'"))
    }
}

/// Emoji rendering policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, Default)]
#[serde(rename_all = "kebab-case")]
pub enum EmojiPolicy {
    /// Follow the host environment and accessibility mode.
    #[default]
    Auto,
    /// Force emoji glyphs on.
    Always,
    /// Disable emoji glyphs.
    Never,
}

impl fmt::Display for EmojiPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Always => write!(f, "always"),
            Self::Never => write!(f, "never"),
        }
    }
}

impl FromStr for EmojiPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <Self as ValueEnum>::from_str(s, true).map_err(|_| format!("invalid emoji policy '{s}'"))
    }
}

/// Accessible-output policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AccessibilityPolicy {
    /// Follow terminal and environment detection.
    #[default]
    Auto,
    /// Force accessible output on.
    On,
    /// Force accessible output off.
    Off,
}

impl fmt::Display for AccessibilityPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::On => write!(f, "on"),
            Self::Off => write!(f, "off"),
        }
    }
}

impl FromStr for AccessibilityPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <Self as ValueEnum>::from_str(s, true)
            .map_err(|_| format!("invalid accessibility policy '{s}'"))
    }
}

/// Layered defaults for the `build` subcommand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BuildConfig {
    /// Default targets used when the user does not pass any targets.
    #[serde(default)]
    pub targets: Vec<String>,
}

/// Subcommand-specific layered defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CommandConfigs {
    /// Configuration that applies only to the `build` subcommand.
    #[serde(default)]
    pub build: BuildConfig,
}

/// Authoritative schema for layered CLI configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "NETSUKE", post_merge_hook)]
pub struct CliConfig {
    /// Path to the Netsuke manifest file to use.
    #[ortho_config(default = default_manifest_path())]
    pub file: PathBuf,

    /// Set the number of parallel build jobs.
    pub jobs: Option<usize>,

    /// Enable verbose diagnostic logging and completion timing summaries.
    #[ortho_config(default = false)]
    pub verbose: bool,

    /// Locale tag for CLI copy (for example: en-US, es-ES).
    pub locale: Option<String>,

    /// Additional URL schemes allowed for the `fetch` helper.
    #[ortho_config(merge_strategy = "append")]
    #[serde(default)]
    pub fetch_allow_scheme: Vec<String>,

    /// Hostnames permitted when default deny is enabled.
    #[ortho_config(merge_strategy = "append")]
    #[serde(default)]
    pub fetch_allow_host: Vec<HostPattern>,

    /// Hostnames that are always blocked.
    #[ortho_config(merge_strategy = "append")]
    #[serde(default)]
    pub fetch_block_host: Vec<HostPattern>,

    /// Deny all hosts by default; only allow the declared allowlist.
    #[ortho_config(default = false)]
    pub fetch_default_deny: bool,

    /// Emit machine-readable JSON output.
    #[ortho_config(default = false)]
    pub json: bool,

    /// Never read interactive input.
    #[ortho_config(skip_cli)]
    pub no_input: NoInput,

    /// Preferred colour policy.
    #[ortho_config(skip_cli)]
    pub color: ColourPolicy,

    /// Preferred emoji policy.
    #[ortho_config(skip_cli)]
    pub emoji: EmojiPolicy,

    /// Preferred progress policy.
    #[ortho_config(skip_cli)]
    pub progress: ProgressPolicy,

    /// Preferred accessibility policy.
    #[ortho_config(skip_cli)]
    pub accessibility: AccessibilityPolicy,

    /// Compatibility alias for default build targets at the config root.
    #[ortho_config(merge_strategy = "append")]
    #[serde(default)]
    pub default_targets: Vec<String>,

    /// Per-subcommand defaults.
    #[ortho_config(skip_cli)]
    #[serde(default)]
    pub cmds: CommandConfigs,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            file: Self::default_manifest_path(),
            jobs: None,
            verbose: false,
            locale: None,
            fetch_allow_scheme: Vec::new(),
            fetch_allow_host: Vec::new(),
            fetch_block_host: Vec::new(),
            fetch_default_deny: false,
            json: false,
            no_input: NoInput::default(),
            color: ColourPolicy::Auto,
            emoji: EmojiPolicy::Auto,
            progress: ProgressPolicy::Auto,
            accessibility: AccessibilityPolicy::Auto,
            default_targets: Vec::new(),
            cmds: CommandConfigs::default(),
        }
    }
}

impl CliConfig {
    pub(super) fn default_manifest_path() -> PathBuf {
        default_manifest_path()
    }
}

const MAX_JOBS: usize = super::parser::MAX_JOBS;

const fn jobs_out_of_bounds(jobs: usize) -> bool {
    jobs == 0 || jobs > MAX_JOBS
}

impl PostMergeHook for CliConfig {
    fn post_merge(&mut self, _ctx: &PostMergeContext) -> OrthoResult<()> {
        validate_non_interactive(self)?;
        validate_jobs(self)?;
        Ok(())
    }
}

fn default_manifest_path() -> PathBuf {
    PathBuf::from("Netsukefile")
}

fn validate_non_interactive(config: &CliConfig) -> OrthoResult<()> {
    if config.no_input.is_enabled() {
        Ok(())
    } else {
        Err(validation_error(
            "no_input",
            "no_input = false is unsupported because Netsuke has no interactive mode",
        ))
    }
}

fn validate_jobs(config: &CliConfig) -> OrthoResult<()> {
    let Some(jobs) = config.jobs else {
        return Ok(());
    };
    if jobs_out_of_bounds(jobs) {
        return Err(validation_error(
            "jobs",
            &format!("jobs = {jobs} is out of range; must be between 1 and {MAX_JOBS}"),
        ));
    }
    Ok(())
}
