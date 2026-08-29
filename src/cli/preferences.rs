//! Runtime preferences derived from parsed CLI flags.
//!
//! These accessors translate the raw policy flags carried by
//! [`Cli`](super::command::Cli) into the preference values the runner and
//! presentation layers consume. They live beside the command schema rather
//! than inside it so that [`super::command`] keeps a dependency surface narrow
//! enough for `build.rs` to compile on its own.

use super::command::Cli;
use super::{AccessibilityPolicy, EmojiPolicy, ProgressPolicy};
use crate::theme::ThemePreference;

impl Cli {
    /// Return the effective theme preference for emoji policy resolution.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::cli::{Cli, EmojiPolicy};
    /// use netsuke::theme::ThemePreference;
    ///
    /// let cli = Cli {
    ///     emoji: EmojiPolicy::Always,
    ///     ..Cli::default()
    /// };
    ///
    /// assert_eq!(cli.theme_preference(), Some(ThemePreference::Unicode));
    /// ```
    #[must_use]
    pub const fn theme_preference(&self) -> Option<ThemePreference> {
        match self.emoji {
            EmojiPolicy::Auto => None,
            EmojiPolicy::Always => Some(ThemePreference::Unicode),
            EmojiPolicy::Never => Some(ThemePreference::Ascii),
        }
    }

    /// Return an explicit accessible-output override, if configured.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::cli::{AccessibilityPolicy, Cli};
    ///
    /// let cli = Cli {
    ///     accessibility: AccessibilityPolicy::On,
    ///     ..Cli::default()
    /// };
    ///
    /// assert_eq!(cli.accessibility_override(), Some(true));
    /// ```
    #[must_use]
    pub const fn accessibility_override(&self) -> Option<bool> {
        match self.accessibility {
            AccessibilityPolicy::Auto => None,
            AccessibilityPolicy::On => Some(true),
            AccessibilityPolicy::Off => Some(false),
        }
    }

    /// Return whether interactive input is disabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::cli::Cli;
    ///
    /// assert!(Cli::default().no_input());
    /// ```
    #[must_use]
    pub const fn no_input(&self) -> bool {
        self.interaction.no_input
    }

    /// Return whether progress summaries should be enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::cli::{Cli, ProgressPolicy};
    ///
    /// let cli = Cli {
    ///     progress: ProgressPolicy::Never,
    ///     ..Cli::default()
    /// };
    ///
    /// assert!(!cli.progress_enabled());
    /// ```
    #[must_use]
    pub const fn progress_enabled(&self) -> bool {
        !matches!(self.progress, ProgressPolicy::Never)
    }
}
