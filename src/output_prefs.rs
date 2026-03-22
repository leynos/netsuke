//! Output preference resolution for emoji and semantic prefix formatting.
//!
//! This module determines whether Netsuke should include emoji glyphs in its
//! output and provides localized semantic prefix helpers that adapt to the
//! resolved theme tokens. Preferences are auto-detected from the `NO_COLOR`
//! and `NETSUKE_NO_EMOJI` environment variables, or forced via explicit
//! configuration.

use std::env;

use crate::localization::{self, LocalizedMessage, keys};
use crate::output_mode::OutputMode;
use crate::theme::{self, ResolvedTheme, ThemePreference};

/// Resolved output formatting preferences.
///
/// These preferences control whether emoji glyphs appear in output and
/// provide semantic prefix formatting for status, error, and success
/// messages.
///
/// This is now a compatibility facade over the theme system introduced in
/// roadmap 3.12.1. Callers still ask for output preferences, while the
/// implementation delegates prefix and spacing decisions to the resolved theme.
///
/// # Examples
///
/// ```
/// use netsuke::output_prefs::{OutputPrefs, resolve_with};
///
/// let prefs = resolve_with(None, |_| None);
/// assert!(prefs.emoji_allowed());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputPrefs {
    resolved_theme: ResolvedTheme,
}

impl OutputPrefs {
    #[must_use]
    const fn from_theme(resolved_theme: ResolvedTheme) -> Self {
        Self { resolved_theme }
    }

    /// Return `true` when emoji glyphs are permitted.
    #[must_use]
    pub const fn emoji_allowed(self) -> bool {
        self.resolved_theme.tokens.emoji_allowed
    }

    /// Return the task-progress indentation string for the active theme.
    #[must_use]
    pub const fn task_indent(self) -> &'static str {
        self.resolved_theme.tokens.spacing.task_indent
    }

    /// Return the timing-summary indentation string for the active theme.
    #[must_use]
    pub const fn timing_indent(self) -> &'static str {
        self.resolved_theme.tokens.spacing.timing_indent
    }

    fn render_prefix(symbol: &'static str, label_key: &'static str) -> LocalizedMessage {
        let label = localization::message(label_key).to_string();
        LocalizedMessage::literal(format!("{symbol} {label}"))
    }

    /// Render the localized error prefix for the current preferences.
    ///
    /// Returns `"✖ Error:"` for the Unicode theme and `"X Error:"` for the
    /// ASCII theme.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::output_prefs::resolve_with;
    ///
    /// let prefs = resolve_with(Some(true), |_| None);
    /// let prefix = prefs.error_prefix().to_string();
    /// assert!(prefix.contains("Error:"));
    /// ```
    #[must_use]
    pub fn error_prefix(self) -> LocalizedMessage {
        Self::render_prefix(
            self.resolved_theme.tokens.symbols.error,
            keys::SEMANTIC_PREFIX_ERROR,
        )
    }

    /// Render the localized warning prefix for the current preferences.
    ///
    /// Returns `"⚠ Warning:"` for the Unicode theme and `"! Warning:"` for
    /// the ASCII theme.
    #[must_use]
    pub fn warning_prefix(self) -> LocalizedMessage {
        Self::render_prefix(
            self.resolved_theme.tokens.symbols.warning,
            keys::SEMANTIC_PREFIX_WARNING,
        )
    }

    /// Render the localized success prefix for the current preferences.
    ///
    /// Returns `"✔ Success:"` for the Unicode theme and `"+ Success:"` for
    /// the ASCII theme.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::output_prefs::resolve_with;
    ///
    /// let prefs = resolve_with(Some(true), |_| None);
    /// let prefix = prefs.success_prefix().to_string();
    /// assert!(prefix.contains("Success:"));
    /// ```
    #[must_use]
    pub fn success_prefix(self) -> LocalizedMessage {
        Self::render_prefix(
            self.resolved_theme.tokens.symbols.success,
            keys::SEMANTIC_PREFIX_SUCCESS,
        )
    }

    /// Render the localized informational prefix for the current preferences.
    ///
    /// Returns `"ℹ Info:"` for the Unicode theme and `"i Info:"` for the
    /// ASCII theme.
    #[must_use]
    pub fn info_prefix(self) -> LocalizedMessage {
        Self::render_prefix(
            self.resolved_theme.tokens.symbols.info,
            keys::SEMANTIC_PREFIX_INFO,
        )
    }

    /// Render the localized timing prefix for the current preferences.
    ///
    /// Returns `"⏱ Timing:"` for the Unicode theme and `"T Timing:"` for the
    /// ASCII theme.
    #[must_use]
    pub fn timing_prefix(self) -> LocalizedMessage {
        Self::render_prefix(
            self.resolved_theme.tokens.symbols.timing,
            keys::SEMANTIC_PREFIX_TIMING,
        )
    }
}

/// Resolve output preferences from theme, output mode, and legacy settings.
///
/// This is the primary resolution function introduced in roadmap 3.12.1.
/// It delegates to [`resolve_from_theme_with`] and ultimately to
/// [`theme::resolve_theme`] while preserving backward compatibility with the
/// legacy `no_emoji` preference. That shared resolution path also honours the
/// `NO_COLOR` environment variable.
///
/// Precedence (highest to lowest):
/// 1. Explicit theme preference (if not `Auto`)
/// 2. Legacy `no_emoji = true`
/// 3. `NETSUKE_NO_EMOJI` environment variable
/// 4. `NO_COLOR` environment variable
/// 5. Output mode (Accessible uses ASCII, Standard uses Unicode)
///
/// # Examples
///
/// ```
/// use netsuke::output_prefs::resolve_from_theme;
/// use netsuke::theme::ThemePreference;
/// use netsuke::output_mode::OutputMode;
///
/// let prefs = resolve_from_theme(
///     Some(ThemePreference::Ascii),
///     None,
///     OutputMode::Standard
/// );
/// assert!(!prefs.emoji_allowed());
/// ```
#[must_use]
pub fn resolve_from_theme(
    theme: Option<ThemePreference>,
    no_emoji: Option<bool>,
    mode: OutputMode,
) -> OutputPrefs {
    resolve_from_theme_with(theme, no_emoji, mode, |key| env::var(key).ok())
}

/// Testable variant of `resolve_from_theme` with custom environment lookup.
#[must_use]
pub fn resolve_from_theme_with<F>(
    theme: Option<ThemePreference>,
    no_emoji: Option<bool>,
    mode: OutputMode,
    read_env: F,
) -> OutputPrefs
where
    F: Fn(&str) -> Option<String>,
{
    let resolved_theme = theme::resolve_theme(theme, no_emoji, mode, read_env);
    OutputPrefs::from_theme(resolved_theme)
}

/// Resolve output preferences from explicit configuration and environment.
///
/// Precedence:
/// 1. Explicit `Some(true)` forces emoji off unconditionally.
/// 2. `NO_COLOR` environment variable (any value, including empty):
///    emoji off.
/// 3. `NETSUKE_NO_EMOJI` environment variable (any value, including empty):
///    emoji off.
/// 4. Default: emoji allowed.
///
/// `Some(false)` does **not** override environment checks — it is treated
/// the same as `None`. Only `Some(true)` acts as a hard override. This
/// ensures that `NETSUKE_NO_EMOJI` (with any value, including `"false"`)
/// always suppresses emoji unless the CLI explicitly passes
/// `--no-emoji true`, which sets `Some(true)` at the CLI merge layer.
///
/// # Examples
///
/// ```
/// use netsuke::output_prefs::{OutputPrefs, resolve};
///
/// // Explicit true forces emoji off.
/// assert!(!resolve(Some(true)).emoji_allowed());
/// // Some(false) falls through to environment / default.
/// assert!(resolve(Some(false)).emoji_allowed());
/// ```
#[must_use]
pub fn resolve(no_emoji: Option<bool>) -> OutputPrefs {
    resolve_with(no_emoji, |key| env::var(key).ok())
}

/// Testable variant that accepts an environment lookup function.
///
/// The `read_env` closure receives an environment variable name and returns
/// `Some(value)` when the variable is set.
///
/// Override semantics:
/// - `Some(true)` — explicit CLI `--no-emoji true`: forces emoji off,
///   bypassing all environment checks.
/// - `Some(false)` — explicit CLI `--no-emoji false`: **does not** re-enable
///   emoji unconditionally. Falls through to environment checks so that
///   presence-based variables (`NETSUKE_NO_EMOJI`) are still honoured.
///   This prevents `NETSUKE_NO_EMOJI=false` (which is still *set*) from
///   being silently overridden when the value originates from environment
///   parsing rather than a deliberate CLI flag.
/// - `None` — no explicit setting: environment checks apply.
///
/// # Examples
///
/// ```
/// use netsuke::output_prefs::{OutputPrefs, resolve_with};
///
/// let prefs = resolve_with(None, |key| match key {
///     "NO_COLOR" => Some(String::from("1")),
///     _ => None,
/// });
/// assert!(!prefs.emoji_allowed());
/// ```
#[must_use]
pub fn resolve_with<F>(no_emoji: Option<bool>, read_env: F) -> OutputPrefs
where
    F: Fn(&str) -> Option<String>,
{
    let resolved_theme = theme::resolve_theme(None, no_emoji, OutputMode::Standard, read_env);
    OutputPrefs::from_theme(resolved_theme)
}

#[cfg(test)]
#[path = "output_prefs_tests.rs"]
mod tests;
