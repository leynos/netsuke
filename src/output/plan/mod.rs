//! Resolve all terminal presentation choices from policies and captured facts.
//!
//! [`crate::output_prefs::OutputPrefs`], [`crate::theme::ThemeContext`],
//! [`crate::theme::ResolvedTheme`], and the runner's `ReporterOptions` each
//! capture one presentation facet. This pure module composes those facets
//! once. Its permitted callers are the binary composition root, `runner`, and
//! tests; I/O adapters consume the resulting [`DisplayPlan`] and never
//! re-derive presentation decisions.

use crate::cli::config::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
use crate::output_mode::{EnvSignals, OutputMode};
use crate::output_prefs::{self, OutputPrefs};
use crate::theme::{ThemeContext, ThemePreference};

/// Policies that control one command's terminal presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayPolicies {
    /// Select automatic, forced, or disabled colour.
    pub colour: ColourPolicy,
    /// Select automatic, forced, or disabled emoji glyphs.
    pub emoji: EmojiPolicy,
    /// Select automatic, forced, or disabled progress reporting.
    pub progress: ProgressPolicy,
    /// Select automatic, accessible, or standard output.
    pub accessibility: AccessibilityPolicy,
    /// Whether the command emits JSON output.
    pub json: bool,
}

/// Terminal and environment facts captured before policy resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalFacts {
    /// Reduced environment values used by presentation policies.
    pub env: EnvSignals,
    /// Whether stdout is connected to a terminal.
    pub stdout_is_terminal: bool,
    /// Whether stderr is connected to a terminal.
    pub stderr_is_terminal: bool,
}

impl TerminalFacts {
    /// Combine environment signals with the observed stream capabilities.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::display_plan::TerminalFacts;
    /// use netsuke::output_mode::EnvSignals;
    ///
    /// let facts = TerminalFacts::from_signals(EnvSignals::default(), true, false);
    /// assert!(facts.stdout_is_terminal);
    /// assert!(!facts.stderr_is_terminal);
    /// ```
    #[must_use]
    pub const fn from_signals(
        env: EnvSignals,
        stdout_is_terminal: bool,
        stderr_is_terminal: bool,
    ) -> Self {
        Self {
            env,
            stdout_is_terminal,
            stderr_is_terminal,
        }
    }
}

/// The status reporter selected by a resolved display plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReporterKind {
    /// Suppress progress reporting.
    Silent,
    /// Render labelled, static progress updates.
    Accessible,
    /// Render standard progress updates, with redraws when supported.
    Standard,
}

/// Text and link capabilities selected for diagnostic rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticStyle {
    /// Whether the diagnostic may contain ANSI colour.
    pub colour: bool,
    /// Whether Unicode symbols are permitted.
    pub unicode: bool,
    /// Whether terminal hyperlinks may be emitted.
    pub links: bool,
}

/// A consistent set of presentation decisions for one command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayPlan {
    /// Accessible or standard terminal output mode.
    output_mode: OutputMode,
    /// Theme-backed symbol and spacing preferences.
    prefs: OutputPrefs,
    /// Selected progress reporter.
    reporter: ReporterKind,
    /// Whether progress may redraw stderr.
    live_progress: bool,
    /// Whether task updates must use plain text.
    force_text_task_updates: bool,
    /// Whether colour is enabled for stderr.
    stderr_colour: bool,
    /// Text and link capabilities selected for diagnostics.
    diagnostic_style: DiagnosticStyle,
}

impl DisplayPlan {
    /// Return the accessible or standard output mode.
    #[must_use]
    pub const fn output_mode(self) -> OutputMode {
        self.output_mode
    }

    /// Return the theme-backed output preferences.
    #[must_use]
    pub const fn prefs(self) -> OutputPrefs {
        self.prefs
    }

    /// Return the selected progress reporter.
    #[must_use]
    pub const fn reporter(self) -> ReporterKind {
        self.reporter
    }

    /// Return whether progress reporting is enabled.
    #[must_use]
    pub const fn progress_enabled(self) -> bool {
        !matches!(self.reporter, ReporterKind::Silent)
    }

    /// Return whether progress may redraw the terminal.
    #[must_use]
    pub const fn live_progress(self) -> bool {
        self.live_progress
    }

    /// Return whether task updates must use plain text.
    #[must_use]
    pub const fn force_text_task_updates(self) -> bool {
        self.force_text_task_updates
    }

    /// Return whether colour is enabled for stderr output.
    #[must_use]
    pub const fn stderr_colour(self) -> bool {
        self.stderr_colour
    }

    /// Return the style selected for diagnostics.
    #[must_use]
    pub const fn diagnostic_style(self) -> DiagnosticStyle {
        self.diagnostic_style
    }
}

/// Resolve a command's policies against its captured terminal facts.
///
/// The resolver performs no I/O. It makes every presentation decision once so
/// reporters and diagnostic adapters can apply a shared, consistent result.
///
/// # Examples
///
/// ```
/// use netsuke::cli::config::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
/// use netsuke::display_plan::{resolve_display_plan, DisplayPolicies, ReporterKind, TerminalFacts};
///
/// let plan = resolve_display_plan(
///     DisplayPolicies {
///         colour: ColourPolicy::Never,
///         emoji: EmojiPolicy::Auto,
///         progress: ProgressPolicy::Auto,
///         accessibility: AccessibilityPolicy::Auto,
///         json: false,
///     },
///     TerminalFacts::default(),
/// );
/// assert_eq!(plan.reporter(), ReporterKind::Accessible);
/// assert!(!plan.stderr_colour());
/// ```
#[must_use]
pub fn resolve_display_plan(policies: DisplayPolicies, facts: TerminalFacts) -> DisplayPlan {
    let output_mode = facts.env.resolve_output_mode(
        accessibility_override(policies.accessibility),
        Some(policies.colour),
    );
    let prefs = output_prefs::resolve_from_theme_with(
        theme_preference(policies.emoji),
        ThemeContext::new(None, Some(policies.colour), output_mode),
        |key| match (key, facts.env.no_color_set) {
            ("NO_COLOR", true) => Some(String::new()),
            _ => None,
        },
    );
    let reporter = reporter_kind(policies, output_mode);
    let stderr_colour = colour_enabled(
        policies.colour,
        facts.stderr_is_terminal,
        facts.env,
        policies.json,
    );
    let live_progress = matches!(reporter, ReporterKind::Standard)
        && facts.stderr_is_terminal
        && facts.env.redraw_capable;

    DisplayPlan {
        output_mode,
        prefs,
        reporter,
        live_progress,
        force_text_task_updates: output_mode.is_accessible() || !facts.stdout_is_terminal,
        stderr_colour,
        diagnostic_style: DiagnosticStyle {
            colour: stderr_colour,
            unicode: prefs.emoji_allowed(),
            links: stderr_colour && !output_mode.is_accessible(),
        },
    }
}

/// Resolve the startup colour hint against stdout's captured facts.
#[must_use]
pub const fn startup_colour(hint: ColourPolicy, facts: TerminalFacts) -> bool {
    colour_enabled(hint, facts.stdout_is_terminal, facts.env, false)
}

/// Resolve whether progress is suppressed or uses the selected output mode.
const fn reporter_kind(policies: DisplayPolicies, output_mode: OutputMode) -> ReporterKind {
    if policies.json || matches!(policies.progress, ProgressPolicy::Never) {
        ReporterKind::Silent
    } else if output_mode.is_accessible() {
        ReporterKind::Accessible
    } else {
        ReporterKind::Standard
    }
}

/// Resolve colour for one stream from policy and captured facts.
const fn colour_enabled(
    policy: ColourPolicy,
    stream_is_terminal: bool,
    env: EnvSignals,
    json: bool,
) -> bool {
    if json {
        return false;
    }
    match policy {
        ColourPolicy::Auto => stream_is_terminal && !env.no_color_set && !env.term_dumb,
        ColourPolicy::Always => true,
        ColourPolicy::Never => false,
    }
}

/// Convert emoji policy into the theme preference consumed by the theme facade.
const fn theme_preference(policy: EmojiPolicy) -> Option<ThemePreference> {
    match policy {
        EmojiPolicy::Auto => None,
        EmojiPolicy::Always => Some(ThemePreference::Unicode),
        EmojiPolicy::Never => Some(ThemePreference::Ascii),
    }
}

/// Convert accessibility policy into the output-mode override.
const fn accessibility_override(policy: AccessibilityPolicy) -> Option<bool> {
    match policy {
        AccessibilityPolicy::Auto => None,
        AccessibilityPolicy::On => Some(true),
        AccessibilityPolicy::Off => Some(false),
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
