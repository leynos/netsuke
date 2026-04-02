//! Typed CLI configuration preferences and compatibility resolution helpers.
//!
//! This module defines the user-facing preference surface that is layered
//! through `OrthoConfig` across defaults, config files, environment variables,
//! and CLI flags.

use clap::Args;
use ortho_config::OrthoConfig;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::theme::ThemePreference;

/// Structured parse error for CLI configuration enums.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseConfigEnumError {
    /// The original raw value that failed to parse.
    pub raw: Box<str>,
    /// Canonical valid option strings for the enum.
    pub valid_options: &'static [&'static str],
}

impl fmt::Display for ParseConfigEnumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid value '{}'. Valid options: {}",
            self.raw,
            self.valid_options.join(", ")
        )
    }
}

impl std::error::Error for ParseConfigEnumError {}

/// Colour output policy for human-readable CLI output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum ColourPolicy {
    /// Honour terminal and environment auto-detection.
    #[default]
    Auto,
    /// Force colour-capable behaviour even when `NO_COLOR` is set.
    Always,
    /// Disable colour-capable behaviour and treat output as `NO_COLOR`.
    Never,
}

impl ColourPolicy {
    /// Canonical list of valid option strings.
    pub const VALID_OPTIONS: &'static [&'static str] = &["auto", "always", "never"];

    /// Parse a raw colour policy value.
    ///
    /// # Errors
    ///
    /// Returns the canonical valid options when parsing fails.
    pub fn parse_raw(s: &str) -> Result<Self, &'static [&'static str]> {
        let trimmed = s.trim().to_ascii_lowercase();
        match trimmed.as_str() {
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            _ => Err(Self::VALID_OPTIONS),
        }
    }
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
    type Err = ParseConfigEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_raw(s).map_err(|valid_options| ParseConfigEnumError {
            raw: s.into(),
            valid_options,
        })
    }
}

impl<'de> Deserialize<'de> for ColourPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_config_enum(deserializer, "colour policy")
    }
}

/// Progress spinner display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum SpinnerMode {
    /// Emit progress updates.
    #[default]
    Enabled,
    /// Suppress progress updates.
    Disabled,
}

impl SpinnerMode {
    /// Canonical list of valid option strings.
    pub const VALID_OPTIONS: &'static [&'static str] = &["enabled", "disabled"];

    /// Parse a raw spinner mode value.
    ///
    /// # Errors
    ///
    /// Returns the canonical valid options when parsing fails.
    pub fn parse_raw(s: &str) -> Result<Self, &'static [&'static str]> {
        let trimmed = s.trim().to_ascii_lowercase();
        match trimmed.as_str() {
            "enabled" => Ok(Self::Enabled),
            "disabled" => Ok(Self::Disabled),
            _ => Err(Self::VALID_OPTIONS),
        }
    }
}

impl fmt::Display for SpinnerMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Enabled => write!(f, "enabled"),
            Self::Disabled => write!(f, "disabled"),
        }
    }
}

impl FromStr for SpinnerMode {
    type Err = ParseConfigEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_raw(s).map_err(|valid_options| ParseConfigEnumError {
            raw: s.into(),
            valid_options,
        })
    }
}

impl<'de> Deserialize<'de> for SpinnerMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_config_enum(deserializer, "spinner mode")
    }
}

/// Diagnostic output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum OutputFormat {
    /// Human-readable diagnostics.
    #[default]
    Human,
    /// Machine-readable JSON diagnostics.
    Json,
}

impl OutputFormat {
    /// Canonical list of valid option strings.
    pub const VALID_OPTIONS: &'static [&'static str] = &["human", "json"];

    /// Parse a raw output format value.
    ///
    /// # Errors
    ///
    /// Returns the canonical valid options when parsing fails.
    pub fn parse_raw(s: &str) -> Result<Self, &'static [&'static str]> {
        let trimmed = s.trim().to_ascii_lowercase();
        match trimmed.as_str() {
            "human" => Ok(Self::Human),
            "json" => Ok(Self::Json),
            _ => Err(Self::VALID_OPTIONS),
        }
    }

    /// Return `true` when JSON diagnostics are enabled.
    #[must_use]
    pub const fn is_json(self) -> bool {
        matches!(self, Self::Json)
    }
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
    type Err = ParseConfigEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_raw(s).map_err(|valid_options| ParseConfigEnumError {
            raw: s.into(),
            valid_options,
        })
    }
}

impl<'de> Deserialize<'de> for OutputFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_config_enum(deserializer, "output format")
    }
}

fn deserialize_config_enum<'de, D, T>(deserializer: D, label: &str) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: fmt::Display,
{
    let raw = String::deserialize(deserializer)?;
    T::from_str(&raw).map_err(|err| de::Error::custom(format!("invalid {label}: {err}")))
}

/// Preference-oriented configuration extracted from the top-level CLI surface.
#[derive(Debug, Clone, PartialEq, Eq, Args, Serialize, Deserialize, OrthoConfig, Default)]
pub struct CliConfig {
    /// Enable verbose diagnostic logging and completion timing summaries.
    #[arg(short, long)]
    #[ortho_config(default = false)]
    pub verbose: bool,

    /// Locale tag for CLI copy (for example: en-US, es-ES).
    #[arg(long, value_name = "LOCALE")]
    pub locale: Option<String>,

    /// Force accessible output mode on or off (overrides auto-detection).
    #[arg(long)]
    pub accessible: Option<bool>,

    /// Suppress emoji glyphs in output (overrides auto-detection).
    #[arg(long)]
    pub no_emoji: Option<bool>,

    /// CLI theme preset (auto, unicode, ascii).
    #[arg(long, value_name = "THEME")]
    pub theme: Option<ThemePreference>,

    /// Colour output policy (auto, always, never).
    #[arg(long, value_name = "POLICY")]
    pub colour_policy: Option<ColourPolicy>,

    /// Force standard progress summaries on or off.
    ///
    /// When omitted, Netsuke enables progress summaries in standard mode.
    #[arg(long)]
    pub progress: Option<bool>,

    /// Spinner display mode (enabled, disabled).
    #[arg(long, value_name = "MODE")]
    pub spinner_mode: Option<SpinnerMode>,

    /// Emit machine-readable diagnostics in JSON on stderr.
    #[arg(long)]
    #[ortho_config(default = false)]
    pub diag_json: bool,

    /// Diagnostic output format (human, json).
    #[arg(long, value_name = "FORMAT")]
    pub output_format: Option<OutputFormat>,

    /// Default build targets used when none are specified on the CLI.
    #[arg(long = "default-target", value_name = "TARGET")]
    #[ortho_config(merge_strategy = "append")]
    pub default_targets: Vec<String>,
}

impl CliConfig {
    /// Resolve whether JSON diagnostics should be active after merge.
    #[must_use]
    pub const fn resolved_diag_json(&self) -> bool {
        match self.output_format {
            Some(output_format) => output_format.is_json(),
            None => self.diag_json,
        }
    }

    /// Resolve whether progress reporting should be active after merge.
    #[must_use]
    pub const fn resolved_progress(&self) -> bool {
        match self.spinner_mode {
            Some(SpinnerMode::Enabled) => true,
            Some(SpinnerMode::Disabled) => false,
            None => match self.progress {
                Some(progress) => progress,
                None => true,
            },
        }
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
