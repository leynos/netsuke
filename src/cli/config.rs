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
use crate::theme::ThemePreference;

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
        <Self as ValueEnum>::from_str(s, true).map_err(|_| format!("invalid colour policy '{s}'"))
    }
}

/// Spinner and progress rendering policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SpinnerMode {
    /// Follow Netsuke's default progress behaviour.
    #[default]
    Auto,
    /// Force progress summaries on.
    Enabled,
    /// Disable progress summaries.
    Disabled,
}

impl fmt::Display for SpinnerMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Enabled => write!(f, "enabled"),
            Self::Disabled => write!(f, "disabled"),
        }
    }
}

impl FromStr for SpinnerMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <Self as ValueEnum>::from_str(s, true).map_err(|_| format!("invalid spinner mode '{s}'"))
    }
}

/// Top-level diagnostics and output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, Default)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    /// Human-readable terminal output.
    #[default]
    Human,
    /// Machine-readable JSON diagnostics.
    Json,
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Human => write!(f, "human"),
            Self::Json => write!(f, "json"),
        }
    }
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <Self as ValueEnum>::from_str(s, true).map_err(|_| format!("invalid output format '{s}'"))
    }
}

/// Presentation theme for semantic prefixes and glyph choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    /// Follow the host environment.
    #[default]
    Auto,
    /// Prefer the Unicode/emoji presentation.
    Unicode,
    /// Prefer ASCII-only output.
    Ascii,
}

impl From<Theme> for ThemePreference {
    fn from(value: Theme) -> Self {
        match value {
            Theme::Auto => Self::Auto,
            Theme::Unicode => Self::Unicode,
            Theme::Ascii => Self::Ascii,
        }
    }
}

impl PartialEq<ThemePreference> for Theme {
    fn eq(&self, other: &ThemePreference) -> bool {
        ThemePreference::from(*self) == *other
    }
}

impl PartialEq<Theme> for ThemePreference {
    fn eq(&self, other: &Theme) -> bool {
        *self == Self::from(*other)
    }
}

/// Layered defaults for the `build` subcommand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BuildConfig {
    /// Optional default path for the emitted Ninja manifest.
    pub emit: Option<PathBuf>,
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

    /// Force accessible output mode on or off.
    pub accessible: Option<bool>,

    /// Compatibility alias for requesting the ASCII theme.
    pub no_emoji: Option<bool>,

    /// Emit machine-readable diagnostics in JSON on stderr.
    #[ortho_config(default = false)]
    pub diag_json: bool,

    /// Force progress summaries on or off.
    pub progress: Option<bool>,

    /// Preferred colour policy.
    #[ortho_config(skip_cli)]
    pub colour_policy: Option<ColourPolicy>,

    /// Preferred spinner or progress mode.
    #[ortho_config(skip_cli)]
    pub spinner_mode: Option<SpinnerMode>,

    /// Preferred diagnostics/output format.
    #[ortho_config(skip_cli)]
    pub output_format: Option<OutputFormat>,

    /// Preferred terminal theme.
    #[ortho_config(skip_cli)]
    pub theme: Option<Theme>,

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
            file: default_manifest_path(),
            jobs: None,
            verbose: false,
            locale: None,
            fetch_allow_scheme: Vec::new(),
            fetch_allow_host: Vec::new(),
            fetch_block_host: Vec::new(),
            fetch_default_deny: false,
            accessible: None,
            no_emoji: None,
            diag_json: false,
            progress: None,
            colour_policy: None,
            spinner_mode: None,
            output_format: None,
            theme: None,
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
        validate_theme_compatibility(self)?;
        validate_spinner_mode_compatibility(self)?;
        validate_jobs(self)?;
        Ok(())
    }
}

fn default_manifest_path() -> PathBuf {
    PathBuf::from("Netsukefile")
}

fn validate_theme_compatibility(config: &CliConfig) -> OrthoResult<()> {
    match (config.theme, config.no_emoji) {
        (Some(Theme::Unicode), Some(true)) => Err(validation_error(
            "theme",
            "theme = \"unicode\" conflicts with no_emoji = true; use theme = \"ascii\" instead",
        )),
        (Some(Theme::Ascii), Some(false)) => Err(validation_error(
            "no_emoji",
            "theme = \"ascii\" conflicts with no_emoji = false; remove the alias or choose theme = \"unicode\"",
        )),
        _ => Ok(()),
    }
}

fn validate_spinner_mode_compatibility(config: &CliConfig) -> OrthoResult<()> {
    match (config.spinner_mode, config.progress) {
        (Some(SpinnerMode::Disabled), Some(true)) => Err(validation_error(
            "spinner_mode",
            "spinner_mode = \"disabled\" conflicts with progress = true",
        )),
        (Some(SpinnerMode::Enabled), Some(false)) => Err(validation_error(
            "progress",
            "spinner_mode = \"enabled\" conflicts with progress = false",
        )),
        _ => Ok(()),
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
