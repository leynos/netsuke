//! Independent expectations for terminal display-policy combinations.
//!
//! These functions encode the documented precedence directly and do not call
//! the display-plan resolver, so the domain sweep can detect resolver drift.

use netsuke::cli::config::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
use netsuke::display_plan::{DiagnosticStyle, DisplayPolicies, ReporterKind, TerminalFacts};
use netsuke::output_mode::OutputMode;

/// One policy combination and its captured environment and stream facts.
#[derive(Debug, Clone, Copy)]
pub(super) struct DisplayCase {
    /// Command policies under test.
    pub policies: DisplayPolicies,
    /// Environment and terminal facts supplied to the resolver.
    pub facts: TerminalFacts,
}

/// Independent expected projections of a display plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ExpectedDisplay {
    /// Accessible or standard output mode.
    pub output_mode: OutputMode,
    /// Whether status glyphs may use Unicode.
    pub emoji_allowed: bool,
    /// Selected progress reporter.
    pub reporter: ReporterKind,
    /// Progress and task-update behaviour.
    pub progress: ExpectedProgress,
    /// Whether stderr may contain colour.
    pub stderr_colour: bool,
    /// Diagnostic renderer capabilities.
    pub diagnostic_style: DiagnosticStyle,
}

/// Independent expected progress and task-update decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ExpectedProgress {
    /// Whether progress reporting is enabled.
    pub enabled: bool,
    /// Whether progress may redraw stderr.
    pub live: bool,
    /// Whether task updates must be rendered as text.
    pub force_text_task_updates: bool,
}

/// Derive the expected display plan without consulting the production resolver.
pub(super) const fn expected_display(case: DisplayCase) -> ExpectedDisplay {
    let output_mode = expected_output_mode(case);
    let emoji_allowed = expected_emoji_allowed(case, output_mode);
    let progress_enabled =
        !case.policies.json && !matches!(case.policies.progress, ProgressPolicy::Never);
    let reporter = if !progress_enabled {
        ReporterKind::Silent
    } else if output_mode.is_accessible() {
        ReporterKind::Accessible
    } else {
        ReporterKind::Standard
    };
    let stderr_colour = expected_colour(
        case.policies.colour,
        case.facts.stderr_is_terminal,
        case.facts,
        case.policies.json,
    );
    let live_progress = matches!(reporter, ReporterKind::Standard)
        && case.facts.stderr_is_terminal
        && case.facts.env.redraw_capable;

    ExpectedDisplay {
        output_mode,
        emoji_allowed,
        reporter,
        progress: ExpectedProgress {
            enabled: progress_enabled,
            live: live_progress,
            force_text_task_updates: output_mode.is_accessible() || !case.facts.stdout_is_terminal,
        },
        stderr_colour,
        diagnostic_style: DiagnosticStyle {
            colour: stderr_colour,
            unicode: emoji_allowed,
            links: stderr_colour && !output_mode.is_accessible(),
        },
    }
}

/// Derive mode from the accessibility, colour, and environment policies.
const fn expected_output_mode(case: DisplayCase) -> OutputMode {
    match case.policies.accessibility {
        AccessibilityPolicy::On => OutputMode::Accessible,
        AccessibilityPolicy::Off => OutputMode::Standard,
        AccessibilityPolicy::Auto => {
            let colour_forces_accessible = match case.policies.colour {
                ColourPolicy::Never => true,
                ColourPolicy::Always => false,
                ColourPolicy::Auto => case.facts.env.no_color_set,
            };
            if colour_forces_accessible || case.facts.env.term_dumb {
                OutputMode::Accessible
            } else {
                OutputMode::Standard
            }
        }
    }
}

/// Derive the Unicode decision from explicit emoji and accessibility policy.
const fn expected_emoji_allowed(case: DisplayCase, output_mode: OutputMode) -> bool {
    match case.policies.emoji {
        EmojiPolicy::Always => true,
        EmojiPolicy::Never => false,
        EmojiPolicy::Auto => {
            let colour_disables_emoji = match case.policies.colour {
                ColourPolicy::Always => false,
                ColourPolicy::Never => true,
                ColourPolicy::Auto => case.facts.env.no_color_set,
            };
            !colour_disables_emoji && !output_mode.is_accessible()
        }
    }
}

/// Derive colour for one stream from the policy and terminal facts.
const fn expected_colour(
    policy: ColourPolicy,
    stream_is_terminal: bool,
    facts: TerminalFacts,
    json: bool,
) -> bool {
    if json {
        return false;
    }
    match policy {
        ColourPolicy::Auto => stream_is_terminal && !facts.env.no_color_set && !facts.env.term_dumb,
        ColourPolicy::Always => true,
        ColourPolicy::Never => false,
    }
}
