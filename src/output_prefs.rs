//! Output preference resolution for emoji and semantic prefix formatting.
//!
//! This module determines whether Netsuke should include emoji glyphs in its
//! output and provides localised semantic prefix helpers (`Error:`,
//! `Warning:`, `Success:`) that adapt to the resolved preference. Preferences
//! are auto-detected from the `NO_COLOR` and `NETSUKE_NO_EMOJI` environment
//! variables, or forced via explicit configuration.

use std::env;

use crate::localization::{self, LocalizedMessage, keys};

/// Resolved output formatting preferences.
///
/// These preferences control whether emoji glyphs appear in output and
/// provide semantic prefix formatting for status, error, and success
/// messages.
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
    /// Whether emoji glyphs are permitted in output.
    emoji: bool,
}

impl OutputPrefs {
    /// Return `true` when emoji glyphs are permitted.
    #[must_use]
    pub const fn emoji_allowed(self) -> bool {
        self.emoji
    }

    /// Fluent argument value for the `$emoji` select expression.
    const fn emoji_arg(self) -> &'static str {
        if self.emoji { "yes" } else { "no" }
    }

    /// Render the localised error prefix for the current preferences.
    ///
    /// Returns `"✖ Error:"` when emoji is allowed, `"Error:"` otherwise.
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
        localization::message(keys::SEMANTIC_PREFIX_ERROR).with_arg("emoji", self.emoji_arg())
    }

    /// Render the localised warning prefix for the current preferences.
    ///
    /// Returns `"⚠ Warning:"` when emoji is allowed, `"Warning:"` otherwise.
    #[must_use]
    pub fn warning_prefix(self) -> LocalizedMessage {
        localization::message(keys::SEMANTIC_PREFIX_WARNING).with_arg("emoji", self.emoji_arg())
    }

    /// Render the localised success prefix for the current preferences.
    ///
    /// Returns `"✔ Success:"` when emoji is allowed, `"Success:"` otherwise.
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
        localization::message(keys::SEMANTIC_PREFIX_SUCCESS).with_arg("emoji", self.emoji_arg())
    }
}

/// Resolve output preferences from explicit configuration and environment.
///
/// Precedence:
/// 1. Explicit configuration (`no_emoji` field): `Some(true)` forces
///    no emoji, `Some(false)` forces emoji on.
/// 2. `NO_COLOR` environment variable (any value, including empty):
///    no emoji.
/// 3. `NETSUKE_NO_EMOJI` environment variable (any value, including empty):
///    no emoji.
/// 4. Default: emoji allowed.
///
/// # Examples
///
/// ```
/// use netsuke::output_prefs::{OutputPrefs, resolve};
///
/// // Explicit configuration takes highest precedence.
/// assert!(!resolve(Some(true)).emoji_allowed());
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
    if let Some(forced) = no_emoji {
        return OutputPrefs { emoji: !forced };
    }

    if read_env("NO_COLOR").is_some() {
        return OutputPrefs { emoji: false };
    }

    if read_env("NETSUKE_NO_EMOJI").is_some() {
        return OutputPrefs { emoji: false };
    }

    OutputPrefs { emoji: true }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// Build an environment lookup from optional `NO_COLOR` and
    /// `NETSUKE_NO_EMOJI` values.
    fn fake_env<'a>(
        no_color: Option<&'a str>,
        no_emoji_env: Option<&'a str>,
    ) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| match key {
            "NO_COLOR" => no_color.map(String::from),
            "NETSUKE_NO_EMOJI" => no_emoji_env.map(String::from),
            _ => None,
        }
    }

    #[rstest]
    #[case::explicit_no_emoji_forces_off(Some(true), None, None, false)]
    #[case::explicit_emoji_on_overrides_no_color(Some(false), Some("1"), None, true)]
    #[case::explicit_emoji_on_overrides_netsuke_no_emoji(Some(false), None, Some("1"), true)]
    #[case::no_color_disables_emoji(None, Some("1"), None, false)]
    #[case::no_color_empty_disables_emoji(None, Some(""), None, false)]
    #[case::netsuke_no_emoji_disables(None, None, Some("1"), false)]
    #[case::netsuke_no_emoji_empty_disables(None, None, Some(""), false)]
    #[case::default_allows_emoji(None, None, None, true)]
    #[case::no_color_takes_precedence_over_missing_netsuke(None, Some("1"), None, false)]
    #[case::both_env_vars_disable(None, Some("1"), Some("1"), false)]
    fn resolve_output_prefs(
        #[case] no_emoji: Option<bool>,
        #[case] no_color: Option<&str>,
        #[case] no_emoji_env: Option<&str>,
        #[case] expected_emoji: bool,
    ) {
        let env = fake_env(no_color, no_emoji_env);
        assert_eq!(resolve_with(no_emoji, env).emoji_allowed(), expected_emoji);
    }

    #[test]
    fn emoji_allowed_returns_true_when_permitted() {
        let prefs = resolve_with(Some(false), |_| None);
        assert!(prefs.emoji_allowed());
    }

    #[test]
    fn emoji_allowed_returns_false_when_suppressed() {
        let prefs = resolve_with(Some(true), |_| None);
        assert!(!prefs.emoji_allowed());
    }

    #[rstest]
    #[case::error_with_emoji(true, OutputPrefs::error_prefix, "Error:")]
    #[case::error_without_emoji(false, OutputPrefs::error_prefix, "Error:")]
    #[case::success_with_emoji(true, OutputPrefs::success_prefix, "Success:")]
    #[case::success_without_emoji(false, OutputPrefs::success_prefix, "Success:")]
    #[case::warning_with_emoji(true, OutputPrefs::warning_prefix, "Warning:")]
    #[case::warning_without_emoji(false, OutputPrefs::warning_prefix, "Warning:")]
    fn prefix_rendering(
        #[case] emoji: bool,
        #[case] prefix_fn: fn(OutputPrefs) -> LocalizedMessage,
        #[case] expected_text: &str,
    ) {
        let prefs = OutputPrefs { emoji };
        let rendered = prefix_fn(prefs).to_string();
        assert!(
            rendered.contains(expected_text),
            "expected '{expected_text}' in '{rendered}'"
        );
        if !emoji {
            assert!(
                rendered.is_ascii(),
                "expected ASCII-only prefix, got '{rendered}'"
            );
        }
    }
}
