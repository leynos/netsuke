//! Runtime preferences derived from parsed CLI flags.
//!
//! These accessors translate the raw policy flags carried by
//! [`Cli`](super::command::Cli) into the preference values the runner and
//! presentation layers consume. They live beside the command schema rather
//! than inside it so that [`super::command`] keeps a dependency surface narrow
//! enough for `build.rs` to compile on its own.

use super::command::Cli;
use super::{AccessibilityPolicy, EmojiPolicy, ProgressPolicy};
use crate::display_plan::DisplayPolicies;
use crate::theme::ThemePreference;

impl AccessibilityPolicy {
    /// Return the forced accessible-mode value, or `None` for automatic mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::cli::AccessibilityPolicy;
    ///
    /// assert_eq!(AccessibilityPolicy::On.explicit_override(), Some(true));
    /// ```
    #[must_use]
    pub const fn explicit_override(self) -> Option<bool> {
        match self {
            Self::Auto => None,
            Self::On => Some(true),
            Self::Off => Some(false),
        }
    }
}

impl EmojiPolicy {
    /// Return the forced Unicode-symbol choice, or `None` for automatic mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::cli::EmojiPolicy;
    ///
    /// assert_eq!(EmojiPolicy::Always.unicode_override(), Some(true));
    /// ```
    #[must_use]
    pub const fn unicode_override(self) -> Option<bool> {
        match self {
            Self::Auto => None,
            Self::Always => Some(true),
            Self::Never => Some(false),
        }
    }
}

impl ProgressPolicy {
    /// Return whether this policy permits progress reporting.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::cli::ProgressPolicy;
    ///
    /// assert!(!ProgressPolicy::Never.is_enabled());
    /// ```
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        !matches!(self, Self::Never)
    }
}

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
        ThemePreference::from_unicode_override(self.emoji.unicode_override())
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
        self.accessibility.explicit_override()
    }

    /// Collect the presentation policies consumed by the display-plan resolver.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::cli::Cli;
    ///
    /// let policies = Cli::default().display_policies();
    /// assert!(!policies.json);
    /// ```
    #[must_use]
    pub const fn display_policies(&self) -> DisplayPolicies {
        DisplayPolicies {
            colour: self.color,
            emoji: self.emoji,
            progress: self.progress,
            accessibility: self.accessibility,
            json: self.json,
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
        self.progress.is_enabled()
    }
}
