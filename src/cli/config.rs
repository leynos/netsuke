//! Typed CLI configuration preferences and compatibility resolution helpers.
//!
//! This module defines the user-facing preference surface that is layered
//! through `OrthoConfig` across defaults, config files, environment variables,
//! and CLI flags.

use clap::Args;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::theme::ThemePreference;

/// Colour output policy for human-readable CLI output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_raw(s)
            .map_err(|valid| format!("invalid value '{s}'. Valid options: {}", valid.join(", ")))
    }
}

/// Progress spinner display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_raw(s)
            .map_err(|valid| format!("invalid value '{s}'. Valid options: {}", valid.join(", ")))
    }
}

/// Diagnostic output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_raw(s)
            .map_err(|valid| format!("invalid value '{s}'. Valid options: {}", valid.join(", ")))
    }
}

/// Preference-oriented configuration extracted from the top-level CLI surface.
#[derive(Debug, Clone, Args, Serialize, Deserialize, OrthoConfig, Default)]
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
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::colour_auto(
        ColourPolicy::parse_raw("auto").map(|value| value.to_string()),
        String::from("auto")
    )]
    #[case::colour_always(
        ColourPolicy::parse_raw("always").map(|value| value.to_string()),
        String::from("always")
    )]
    #[case::spinner_enabled(
        SpinnerMode::parse_raw("enabled").map(|value| value.to_string()),
        String::from("enabled")
    )]
    #[case::spinner_disabled(
        SpinnerMode::parse_raw("disabled").map(|value| value.to_string()),
        String::from("disabled")
    )]
    #[case::output_human(
        OutputFormat::parse_raw("human").map(|value| value.to_string()),
        String::from("human")
    )]
    #[case::output_json(
        OutputFormat::parse_raw("json").map(|value| value.to_string()),
        String::from("json")
    )]
    fn config_enums_round_trip(
        #[case] parsed: Result<String, &'static [&'static str]>,
        #[case] expected: String,
    ) {
        assert_eq!(parsed.expect("enum value should parse"), expected);
    }

    #[rstest]
    #[case::colour(
        "loud",
        ColourPolicy::from_str("loud").expect_err("invalid colour policy should fail"),
        "auto, always, never"
    )]
    #[case::spinner(
        "paused",
        SpinnerMode::from_str("paused").expect_err("invalid spinner mode should fail"),
        "enabled, disabled"
    )]
    #[case::output(
        "tap",
        OutputFormat::from_str("tap").expect_err("invalid output format should fail"),
        "human, json"
    )]
    fn config_enums_reject_invalid_values(
        #[case] raw: &str,
        #[case] error: String,
        #[case] options: &str,
    ) {
        assert!(error.contains(raw));
        assert!(error.contains(options));
    }

    #[test]
    fn cli_config_alias_resolution_prefers_new_fields() {
        let config = CliConfig {
            progress: Some(false),
            spinner_mode: Some(SpinnerMode::Enabled),
            diag_json: false,
            output_format: Some(OutputFormat::Json),
            ..CliConfig::default()
        };

        assert!(config.resolved_progress());
        assert!(config.resolved_diag_json());
    }
}
