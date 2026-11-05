//! Output mode detection for accessible terminal output.
//!
//! This module determines whether Netsuke should produce accessible (static
//! text) or standard (potentially animated) terminal output. Accessible mode
//! is auto-detected from the `NO_COLOR` and `TERM` environment variables, or
//! forced via explicit configuration.

use crate::cli::ColourPolicy;

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

/// Environment facts shared by output-mode and theme resolution.
///
/// This type reduces the raw `NO_COLOR` and `TERM` values once so downstream
/// resolvers can make consistent decisions without reading environment strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnvSignals {
    /// Whether `NO_COLOR` is set to a non-empty value.
    pub no_color_set: bool,
    /// Whether `TERM` is exactly `dumb`.
    pub term_dumb: bool,
    /// Whether live terminal redraws are supported by the platform facts.
    pub redraw_capable: bool,
}

impl EnvSignals {
    /// Read environment strings and reduce them to shared output facts.
    ///
    /// Unix platforms require a set, non-`dumb` `TERM` for redraw capability;
    /// other platforms only exclude `TERM=dumb`.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::output_mode::EnvSignals;
    ///
    /// let signals = EnvSignals::read(|key| match key {
    ///     "NO_COLOR" => Some(String::from("1")),
    ///     "TERM" => Some(String::from("xterm-256color")),
    ///     _ => None,
    /// });
    /// assert!(signals.no_color_set);
    /// assert!(!signals.term_dumb);
    /// ```
    #[must_use]
    pub fn read(read_env: impl Fn(&str) -> Option<String>) -> Self {
        let no_color_set = read_env("NO_COLOR").is_some_and(|value| !value.is_empty());
        let term = read_env("TERM");
        let term_dumb = term.as_deref() == Some("dumb");
        #[cfg(unix)]
        let redraw_capable = term.is_some() && !term_dumb;
        #[cfg(not(unix))]
        let redraw_capable = !term_dumb;

        Self {
            no_color_set,
            term_dumb,
            redraw_capable,
        }
    }

    /// Return whether the colour policy treats `NO_COLOR` as active.
    ///
    /// `always` ignores the environment signal, while `never` acts like an
    /// active signal so existing accessibility behavior remains consistent.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::output_mode::EnvSignals;
    ///
    /// assert!(EnvSignals {
    ///     no_color_set: true,
    ///     ..EnvSignals::default()
    /// }
    /// .no_color_active(None));
    /// ```
    #[must_use]
    pub const fn no_color_active(self, colour_policy: Option<ColourPolicy>) -> bool {
        match colour_policy {
            Some(ColourPolicy::Always) => false,
            Some(ColourPolicy::Never) => true,
            Some(ColourPolicy::Auto) | None => self.no_color_set,
        }
    }

    /// Resolve accessible or standard output mode from these facts.
    ///
    /// Explicit configuration takes precedence, followed by active `NO_COLOR`,
    /// `TERM=dumb`, and the standard-mode default.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::output_mode::{EnvSignals, OutputMode};
    ///
    /// assert_eq!(
    ///     EnvSignals::default().resolve_output_mode(None, None),
    ///     OutputMode::Standard,
    /// );
    /// ```
    #[must_use]
    pub const fn resolve_output_mode(
        self,
        explicit: Option<bool>,
        colour_policy: Option<ColourPolicy>,
    ) -> OutputMode {
        if let Some(forced) = explicit {
            return if forced {
                OutputMode::Accessible
            } else {
                OutputMode::Standard
            };
        }

        if self.no_color_active(colour_policy) || self.term_dumb {
            OutputMode::Accessible
        } else {
            OutputMode::Standard
        }
    }
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
    EnvSignals::read(read_env).resolve_output_mode(explicit, colour_policy)
}

#[cfg(test)]
#[path = "mode/tests.rs"]
mod tests;
