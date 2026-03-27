//! Design token resolution for command-line interface (CLI) theme presentation.
//!
//! This module defines the theme system that controls symbols, spacing, and
//! semantic colours in CLI output. Themes are resolved from layered
//! configuration (CLI > environment > config > defaults) using the `OrthoConfig`
//! model.

use std::fmt;
use std::str::FromStr;

use crate::cli::config::ColourPolicy;
use crate::output_mode::OutputMode;
use serde::{Deserialize, Serialize};

/// User-facing theme preference for CLI presentation.
///
/// Determines whether output uses Unicode symbols or ASCII-safe alternatives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    /// Automatically detect the appropriate theme based on output mode and
    /// environment signals.
    #[default]
    Auto,
    /// Use Unicode symbols (✔, ✖, ⚠, ℹ, ⏱) for status indicators.
    Unicode,
    /// Use ASCII-safe symbols for maximum compatibility.
    Ascii,
}

impl ThemePreference {
    /// Canonical list of valid theme option strings.
    pub const VALID_OPTIONS: &'static [&'static str] = &["auto", "unicode", "ascii"];

    /// Shared parsing logic for theme strings.
    ///
    /// Returns `Ok(ThemePreference)` on success, or `Err(Self::VALID_OPTIONS)`
    /// on failure so callers can construct localised error messages using the
    /// same canonical list of valid options.
    ///
    /// # Errors
    ///
    /// Returns `Err(Self::VALID_OPTIONS)` if the input string does not match
    /// any valid theme option (case-insensitive).
    pub fn parse_raw(s: &str) -> Result<Self, &'static [&'static str]> {
        let trimmed = s.trim().to_ascii_lowercase();
        match trimmed.as_str() {
            "auto" => Ok(Self::Auto),
            "unicode" => Ok(Self::Unicode),
            "ascii" => Ok(Self::Ascii),
            _ => Err(Self::VALID_OPTIONS),
        }
    }
}

impl fmt::Display for ThemePreference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Unicode => write!(f, "unicode"),
            Self::Ascii => write!(f, "ascii"),
        }
    }
}

impl FromStr for ThemePreference {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_raw(s)
            .map_err(|valid| format!("invalid theme '{s}'. Valid options: {}", valid.join(", ")))
    }
}

/// Semantic colour tokens for CLI output.
///
/// These tokens represent semantic intent (error, success, etc.) rather than
/// concrete ANSI colour codes. A future milestone may map these to actual
/// terminal styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColourTokens {
    /// Colour token for error messages and failed operations.
    pub error: SemanticColour,
    /// Colour token for warning messages.
    pub warning: SemanticColour,
    /// Colour token for successful operations.
    pub success: SemanticColour,
    /// Colour token for informational messages.
    pub info: SemanticColour,
    /// Colour token for timing summaries.
    pub timing: SemanticColour,
}

/// Semantic colour identifiers.
///
/// Roadmap item 3.12.1 defines these as data; a later milestone will map them
/// to concrete ANSI styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticColour {
    /// Error or failure state.
    Error,
    /// Warning or caution state.
    Warning,
    /// Success or completion state.
    Success,
    /// Informational or neutral state.
    Info,
    /// Timing or performance data.
    Timing,
}

/// Symbol tokens for status indicators.
///
/// These tokens control which glyphs appear in prefixes and progress output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolTokens {
    /// Symbol for error messages (✖ or X).
    pub error: &'static str,
    /// Symbol for warning messages (⚠ or !).
    pub warning: &'static str,
    /// Symbol for success messages (✔ or +).
    pub success: &'static str,
    /// Symbol for informational messages (ℹ or i).
    pub info: &'static str,
    /// Symbol for timing summaries (⏱ or T).
    pub timing: &'static str,
}

/// Spacing tokens for indentation and layout.
///
/// These tokens centralize spacing decisions so reporters stay consistent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpacingTokens {
    /// Indentation for task progress under stage headers (currently 2 spaces).
    pub task_indent: &'static str,
    /// Indentation for timing detail lines (currently 2 spaces).
    pub timing_indent: &'static str,
}

/// Complete design token set for a resolved theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesignTokens {
    /// Semantic colour tokens.
    pub colours: ColourTokens,
    /// Symbol tokens for status indicators.
    pub symbols: SymbolTokens,
    /// Spacing tokens for layout consistency.
    pub spacing: SpacingTokens,
    /// Whether Unicode symbols (emoji) are allowed in this theme.
    pub emoji_allowed: bool,
}

/// Resolved theme including all design tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedTheme {
    /// The complete token set for this theme.
    pub tokens: DesignTokens,
}

/// Runtime inputs that influence theme resolution beyond the explicit theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeContext {
    /// Legacy no-emoji override.
    pub no_emoji: Option<bool>,
    /// Colour policy override for `NO_COLOR` handling.
    pub colour_policy: Option<ColourPolicy>,
    /// Resolved output mode.
    pub mode: OutputMode,
}

impl ThemeContext {
    /// Create a new theme-resolution context.
    #[must_use]
    pub const fn new(
        no_emoji: Option<bool>,
        colour_policy: Option<ColourPolicy>,
        mode: OutputMode,
    ) -> Self {
        Self {
            no_emoji,
            colour_policy,
            mode,
        }
    }
}

/// Unicode symbol tokens.
const UNICODE_SYMBOLS: SymbolTokens = SymbolTokens {
    error: "✖",
    warning: "⚠",
    success: "✔",
    info: "ℹ",
    timing: "⏱",
};

/// ASCII-safe symbol tokens.
const ASCII_SYMBOLS: SymbolTokens = SymbolTokens {
    error: "X",
    warning: "!",
    success: "+",
    info: "i",
    timing: "T",
};

/// Standard spacing tokens used for all themes.
const SPACING: SpacingTokens = SpacingTokens {
    task_indent: "  ",
    timing_indent: "  ",
};

/// Standard semantic colours (placeholder for future styling).
const COLOURS: ColourTokens = ColourTokens {
    error: SemanticColour::Error,
    warning: SemanticColour::Warning,
    success: SemanticColour::Success,
    info: SemanticColour::Info,
    timing: SemanticColour::Timing,
};

/// Environment signals that may force ASCII symbols.
#[derive(Debug, Clone, Copy)]
struct EnvSignals {
    no_emoji: bool,
    no_color: bool,
}

fn read_no_color_with_policy<F>(colour_policy: Option<ColourPolicy>, read_env: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    match colour_policy {
        Some(ColourPolicy::Always) => false,
        Some(ColourPolicy::Never) => true,
        Some(ColourPolicy::Auto) | None => read_env("NO_COLOR").is_some(),
    }
}

/// Determine whether Unicode symbols should be used based on theme configuration.
///
/// This helper encapsulates the precedence logic for symbol selection:
/// 1. Explicit `theme` preference (if not `Auto`) takes highest precedence
/// 2. Legacy `no_emoji = true` forces ASCII
/// 3. `NETSUKE_NO_EMOJI` environment variable forces ASCII
/// 4. `NO_COLOR` environment variable forces ASCII
/// 5. Output mode: `Accessible` uses ASCII, `Standard` uses Unicode
const fn should_use_unicode(
    theme: Option<ThemePreference>,
    no_emoji: Option<bool>,
    env: EnvSignals,
    mode: OutputMode,
) -> bool {
    match theme {
        Some(ThemePreference::Unicode) => true,
        Some(ThemePreference::Ascii) => false,
        Some(ThemePreference::Auto) | None => {
            // Legacy no_emoji=true forces ASCII
            if let Some(true) = no_emoji {
                return false;
            }
            if env.no_emoji {
                return false;
            }
            if env.no_color {
                return false;
            }
            // Default: Unicode for Standard mode, ASCII for Accessible
            !mode.is_accessible()
        }
    }
}

/// Resolve a theme from configuration, environment, and output mode.
///
/// Precedence order:
/// 1. Explicit `theme` preference (if not `Auto`)
/// 2. Legacy `no_emoji = true` forces ASCII
/// 3. `NETSUKE_NO_EMOJI` environment variable forces ASCII
/// 4. `NO_COLOR` environment variable forces ASCII
/// 5. Output mode: `Accessible` uses ASCII, `Standard` uses Unicode
///
/// # Examples
///
/// ```
/// use netsuke::theme::{ThemeContext, ThemePreference, resolve_theme};
/// use netsuke::output_mode::OutputMode;
///
/// let theme = resolve_theme(
///     Some(ThemePreference::Ascii),
///     ThemeContext::new(None, None, OutputMode::Standard),
///     |_| None
/// );
/// assert_eq!(theme.tokens.symbols.success, "+");
/// ```
#[must_use]
pub fn resolve_theme<F>(
    theme: Option<ThemePreference>,
    context: ThemeContext,
    read_env: F,
) -> ResolvedTheme
where
    F: Fn(&str) -> Option<String>,
{
    let env = EnvSignals {
        no_emoji: read_env("NETSUKE_NO_EMOJI").is_some(),
        no_color: read_no_color_with_policy(context.colour_policy, &read_env),
    };
    let use_unicode = should_use_unicode(theme, context.no_emoji, env, context.mode);

    let symbols = if use_unicode {
        UNICODE_SYMBOLS
    } else {
        ASCII_SYMBOLS
    };

    ResolvedTheme {
        tokens: DesignTokens {
            colours: COLOURS,
            symbols,
            spacing: SPACING,
            emoji_allowed: use_unicode,
        },
    }
}

#[cfg(test)]
#[expect(
    clippy::too_many_arguments,
    reason = "rstest parameterized tests need multiple parameters"
)]
#[path = "theme_tests.rs"]
mod tests;
