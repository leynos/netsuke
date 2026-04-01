//! Output mode detection for accessible terminal output.
//!
//! This module determines whether Netsuke should produce accessible (static
//! text) or standard (potentially animated) terminal output. Accessible mode
//! is auto-detected from the `NO_COLOR` and `TERM` environment variables, or
//! forced via explicit configuration.

use crate::cli::config::ColourPolicy;
use std::env;

/// Whether terminal output should use accessible (static text) or standard
/// (potentially animated) formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Static text output with explicit labels. Suitable for screen readers,
    /// dumb terminals, and CI environments.
    Accessible,
    /// Standard terminal output. May include animated progress indicators
    /// when future features are added.
    Standard,
}

impl OutputMode {
    /// Return `true` when the mode is [`Accessible`](OutputMode::Accessible).
    #[must_use]
    pub const fn is_accessible(self) -> bool {
        matches!(self, Self::Accessible)
    }
}

/// Resolve the output mode from explicit configuration and environment.
///
/// Precedence:
/// 1. Explicit configuration (`accessible` field): `Some(true)` forces
///    [`Accessible`](OutputMode::Accessible), `Some(false)` forces
///    [`Standard`](OutputMode::Standard).
/// 2. `NO_COLOR` environment variable (any value, including empty):
///    [`Accessible`](OutputMode::Accessible).
/// 3. `TERM=dumb`: [`Accessible`](OutputMode::Accessible).
/// 4. Default: [`Standard`](OutputMode::Standard).
///
/// # Examples
///
/// ```
/// use netsuke::output_mode::{OutputMode, resolve};
///
/// // Explicit configuration takes highest precedence.
/// assert_eq!(resolve(Some(true), None), OutputMode::Accessible);
/// assert_eq!(resolve(Some(false), None), OutputMode::Standard);
/// ```
#[must_use]
pub fn resolve(explicit: Option<bool>, colour_policy: Option<ColourPolicy>) -> OutputMode {
    resolve_with(explicit, colour_policy, |key| env::var(key).ok())
}

/// Testable variant that accepts an environment lookup function.
///
/// The `read_env` closure receives an environment variable name and returns
/// `Some(value)` when the variable is set.
///
/// # Examples
///
/// ```
/// use netsuke::output_mode::{OutputMode, resolve_with};
///
/// let mode = resolve_with(None, None, |key| match key {
///     "NO_COLOR" => Some(String::from("1")),
///     _ => None,
/// });
/// assert_eq!(mode, OutputMode::Accessible);
/// ```
#[must_use]
pub fn resolve_with<F>(
    explicit: Option<bool>,
    colour_policy: Option<ColourPolicy>,
    read_env: F,
) -> OutputMode
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(forced) = explicit {
        return if forced {
            OutputMode::Accessible
        } else {
            OutputMode::Standard
        };
    }

    if no_color_active_with(colour_policy, &read_env) {
        return OutputMode::Accessible;
    }

    if read_env("TERM").as_deref() == Some("dumb") {
        return OutputMode::Accessible;
    }

    OutputMode::Standard
}

/// Resolve whether `NO_COLOR` should be treated as active under the configured
/// colour policy.
pub(crate) fn no_color_active_with<F>(colour_policy: Option<ColourPolicy>, read_env: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    match colour_policy {
        Some(ColourPolicy::Always) => false,
        Some(ColourPolicy::Never) => true,
        Some(ColourPolicy::Auto) | None => read_env("NO_COLOR").is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// Build an environment lookup from optional `NO_COLOR` and `TERM` values.
    fn fake_env<'a>(
        no_color: Option<&'a str>,
        term: Option<&'a str>,
    ) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| match key {
            "NO_COLOR" => no_color.map(String::from),
            "TERM" => term.map(String::from),
            _ => None,
        }
    }

    #[rstest]
    #[case::explicit_true_forces_accessible(
        Some(true),
        None,
        (None, None),
        OutputMode::Accessible
    )]
    #[case::explicit_false_overrides_all(
        Some(false),
        None,
        (Some("1"), Some("dumb")),
        OutputMode::Standard
    )]
    #[case::no_color_triggers_accessible(
        None,
        None,
        (Some("1"), None),
        OutputMode::Accessible
    )]
    #[case::term_dumb_triggers_accessible(
        None,
        None,
        (None, Some("dumb")),
        OutputMode::Accessible
    )]
    #[case::normal_term_stays_standard(
        None,
        None,
        (None, Some("xterm-256color")),
        OutputMode::Standard
    )]
    #[case::no_env_defaults_to_standard(
        None,
        None,
        (None, None),
        OutputMode::Standard
    )]
    #[case::empty_no_color_triggers_accessible(
        None,
        None,
        (Some(""), None),
        OutputMode::Accessible
    )]
    #[case::no_color_takes_precedence_over_term(
        None,
        None,
        (Some("1"), Some("xterm-256color")),
        OutputMode::Accessible
    )]
    #[case::explicit_false_overrides_no_color(
        Some(false),
        None,
        (Some("1"), None),
        OutputMode::Standard
    )]
    #[case::explicit_true_without_env(
        Some(true),
        None,
        (None, Some("xterm-256color")),
        OutputMode::Accessible
    )]
    #[case::colour_policy_always_ignores_no_color(
        None,
        Some(ColourPolicy::Always),
        (Some("1"), Some("xterm-256color")),
        OutputMode::Standard
    )]
    #[case::colour_policy_never_forces_accessible(
        None,
        Some(ColourPolicy::Never),
        (None, Some("xterm-256color")),
        OutputMode::Accessible
    )]
    fn resolve_output_mode(
        #[case] explicit: Option<bool>,
        #[case] colour_policy: Option<ColourPolicy>,
        #[case] env_values: (Option<&str>, Option<&str>),
        #[case] expected: OutputMode,
    ) {
        let (no_color, term) = env_values;
        let env = fake_env(no_color, term);
        assert_eq!(resolve_with(explicit, colour_policy, env), expected);
    }

    #[test]
    fn is_accessible_returns_true_for_accessible() {
        assert!(OutputMode::Accessible.is_accessible());
    }

    #[test]
    fn is_accessible_returns_false_for_standard() {
        assert!(!OutputMode::Standard.is_accessible());
    }
}
