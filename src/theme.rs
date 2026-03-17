//! Design token resolution for CLI theme presentation.
//!
//! This module defines the theme system that controls symbols, spacing, and
//! semantic colors in CLI output. Themes are resolved from layered
//! configuration (CLI > environment > config > defaults) using the `OrthoConfig`
//! model.

use std::fmt;
use std::str::FromStr;

use crate::output_mode::OutputMode;
use serde::{Deserialize, Serialize};

/// User-facing theme preference for CLI presentation.
///
/// Determines whether output uses Unicode symbols or ASCII-safe alternatives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    /// Automatically detect the appropriate theme based on output mode and
    /// environment signals.
    Auto,
    /// Use Unicode symbols (✔, ✖, ⚠, ℹ, ⏱) for status indicators.
    Unicode,
    /// Use ASCII-safe symbols for maximum compatibility.
    Ascii,
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

impl Default for ThemePreference {
    fn default() -> Self {
        Self::Auto
    }
}

impl FromStr for ThemePreference {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim().to_ascii_lowercase();
        match trimmed.as_str() {
            "auto" => Ok(Self::Auto),
            "unicode" => Ok(Self::Unicode),
            "ascii" => Ok(Self::Ascii),
            _ => Err(format!(
                "invalid theme '{s}'. Valid options: auto, unicode, ascii"
            )),
        }
    }
}

/// Semantic color tokens for CLI output.
///
/// These tokens represent semantic intent (error, success, etc.) rather than
/// concrete ANSI color codes. A future milestone may map these to actual
/// terminal styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColourTokens {
    /// Color token for error messages and failed operations.
    pub error: SemanticColour,
    /// Color token for warning messages.
    pub warning: SemanticColour,
    /// Color token for successful operations.
    pub success: SemanticColour,
    /// Color token for informational messages.
    pub info: SemanticColour,
    /// Color token for timing summaries.
    pub timing: SemanticColour,
}

/// Semantic color identifiers.
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpacingTokens {
    /// Indentation for task progress under stage headers (currently 2 spaces).
    pub task_indent: &'static str,
    /// Indentation for timing detail lines (currently 2 spaces).
    pub timing_indent: &'static str,
}

/// Complete design token set for a resolved theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignTokens {
    /// Semantic color tokens.
    pub colours: ColourTokens,
    /// Symbol tokens for status indicators.
    pub symbols: SymbolTokens,
    /// Spacing tokens for layout consistency.
    pub spacing: SpacingTokens,
}

/// Resolved theme including all design tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTheme {
    /// The complete token set for this theme.
    pub tokens: DesignTokens,
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

/// Standard semantic colors (placeholder for future styling).
const COLOURS: ColourTokens = ColourTokens {
    error: SemanticColour::Error,
    warning: SemanticColour::Warning,
    success: SemanticColour::Success,
    info: SemanticColour::Info,
    timing: SemanticColour::Timing,
};

/// Resolve a theme from configuration, environment, and output mode.
///
/// Precedence order:
/// 1. Explicit `theme` preference (if not `Auto`)
/// 2. Legacy `no_emoji = true` forces ASCII
/// 3. `NETSUKE_NO_EMOJI` environment variable forces ASCII
/// 4. Output mode: `Accessible` uses ASCII, `Standard` uses Unicode
///
/// # Examples
///
/// ```
/// use netsuke::theme::{ThemePreference, resolve_theme};
/// use netsuke::output_mode::OutputMode;
///
/// let theme = resolve_theme(
///     Some(ThemePreference::Ascii),
///     None,
///     OutputMode::Standard,
///     |_| None
/// );
/// assert_eq!(theme.tokens.symbols.success, "+");
/// ```
#[must_use]
pub fn resolve_theme<F>(
    theme: Option<ThemePreference>,
    no_emoji: Option<bool>,
    mode: OutputMode,
    read_env: F,
) -> ResolvedTheme
where
    F: Fn(&str) -> Option<String>,
{
    // Explicit theme preference (non-Auto) takes highest precedence.
    let use_unicode = match theme {
        Some(ThemePreference::Unicode) => true,
        Some(ThemePreference::Ascii) => false,
        Some(ThemePreference::Auto) | None => {
            // Legacy no_emoji=true forces ASCII
            if let Some(true) = no_emoji {
                false
            } else if read_env("NETSUKE_NO_EMOJI").is_some() {
                false
            } else {
                // Default: Unicode for Standard mode, ASCII for Accessible
                !mode.is_accessible()
            }
        }
    };

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
        },
    }
}

#[cfg(test)]
#[expect(
    clippy::too_many_arguments,
    reason = "rstest parameterized tests need multiple parameters"
)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn fake_env(netsuke_no_emoji: Option<&str>) -> impl Fn(&str) -> Option<String> + '_ {
        move |key| match key {
            "NETSUKE_NO_EMOJI" => netsuke_no_emoji.map(String::from),
            _ => None,
        }
    }

    #[rstest]
    #[case::explicit_unicode_overrides_all(
        Some(ThemePreference::Unicode),
        Some(true),
        Some("1"),
        OutputMode::Accessible,
        true
    )]
    #[case::explicit_ascii_overrides_all(
        Some(ThemePreference::Ascii),
        Some(false),
        None,
        OutputMode::Standard,
        false
    )]
    #[case::auto_defers_to_no_emoji_true(
        Some(ThemePreference::Auto),
        Some(true),
        None,
        OutputMode::Standard,
        false
    )]
    #[case::auto_defers_to_env(
        Some(ThemePreference::Auto),
        None,
        Some("1"),
        OutputMode::Standard,
        false
    )]
    #[case::none_defers_to_no_emoji_true(None, Some(true), None, OutputMode::Standard, false)]
    #[case::none_defers_to_env(None, None, Some("1"), OutputMode::Standard, false)]
    #[case::accessible_mode_uses_ascii(None, None, None, OutputMode::Accessible, false)]
    #[case::standard_mode_uses_unicode(None, None, None, OutputMode::Standard, true)]
    #[case::no_emoji_false_defers_to_mode(None, Some(false), None, OutputMode::Standard, true)]
    fn theme_resolution_precedence(
        #[case] theme: Option<ThemePreference>,
        #[case] no_emoji: Option<bool>,
        #[case] env_no_emoji: Option<&str>,
        #[case] mode: OutputMode,
        #[case] expect_unicode: bool,
    ) {
        let resolved = resolve_theme(theme, no_emoji, mode, fake_env(env_no_emoji));
        if expect_unicode {
            assert_eq!(resolved.tokens.symbols.success, UNICODE_SYMBOLS.success);
            assert_eq!(resolved.tokens.symbols.error, UNICODE_SYMBOLS.error);
        } else {
            assert_eq!(resolved.tokens.symbols.success, ASCII_SYMBOLS.success);
            assert_eq!(resolved.tokens.symbols.error, ASCII_SYMBOLS.error);
        }
    }

    #[test]
    fn spacing_tokens_are_identical_across_themes() {
        let unicode_theme = resolve_theme(
            Some(ThemePreference::Unicode),
            None,
            OutputMode::Standard,
            |_| None,
        );
        let ascii_theme = resolve_theme(
            Some(ThemePreference::Ascii),
            None,
            OutputMode::Standard,
            |_| None,
        );
        assert_eq!(
            unicode_theme.tokens.spacing, ascii_theme.tokens.spacing,
            "spacing must be identical between Unicode and ASCII themes"
        );
    }

    #[test]
    fn unicode_symbols_contain_non_ascii() {
        assert!(!UNICODE_SYMBOLS.success.is_ascii());
        assert!(!UNICODE_SYMBOLS.error.is_ascii());
    }

    #[test]
    fn ascii_symbols_are_ascii_only() {
        assert!(ASCII_SYMBOLS.success.is_ascii());
        assert!(ASCII_SYMBOLS.error.is_ascii());
        assert!(ASCII_SYMBOLS.warning.is_ascii());
        assert!(ASCII_SYMBOLS.info.is_ascii());
        assert!(ASCII_SYMBOLS.timing.is_ascii());
    }
}
