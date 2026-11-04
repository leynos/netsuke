//! Exhaustive verification of the consolidated terminal display plan.
//!
//! Every policy tuple and captured fact is checked against the independent
//! model in [`super::display_policy_truth`]. The sweep also verifies that
//! `auto` and `always` progress policies resolve identically.

use super::display_policy_truth::{DisplayCase, expected_display};
use itertools::iproduct;
use netsuke::cli::config::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
use netsuke::display_plan::{DisplayPolicies, ReporterKind, TerminalFacts, resolve_display_plan};
use netsuke::output_mode::{EnvSignals, OutputMode};

const EMOJI_POLICIES: [EmojiPolicy; 3] =
    [EmojiPolicy::Auto, EmojiPolicy::Always, EmojiPolicy::Never];
const COLOUR_POLICIES: [ColourPolicy; 3] = [
    ColourPolicy::Auto,
    ColourPolicy::Always,
    ColourPolicy::Never,
];
const PROGRESS_POLICIES: [ProgressPolicy; 3] = [
    ProgressPolicy::Auto,
    ProgressPolicy::Always,
    ProgressPolicy::Never,
];
const ACCESSIBILITY_POLICIES: [AccessibilityPolicy; 3] = [
    AccessibilityPolicy::Auto,
    AccessibilityPolicy::On,
    AccessibilityPolicy::Off,
];
const BOOLEAN_VALUES: [bool; 2] = [false, true];
const DOMAIN_SIZE: usize = 5_184;

#[test]
fn exhaustive_domain_sweep_matches_truth_model_and_auto_equals_always() {
    let mut sweep = DomainSweep::default();

    for (
        emoji,
        colour,
        progress,
        accessibility,
        no_color_set,
        term_dumb,
        redraw_capable,
        stdout_is_terminal,
        stderr_is_terminal,
        json,
    ) in iproduct!(
        EMOJI_POLICIES,
        COLOUR_POLICIES,
        PROGRESS_POLICIES,
        ACCESSIBILITY_POLICIES,
        BOOLEAN_VALUES,
        BOOLEAN_VALUES,
        BOOLEAN_VALUES,
        BOOLEAN_VALUES,
        BOOLEAN_VALUES,
        BOOLEAN_VALUES
    ) {
        sweep.observe(DisplayCase {
            policies: DisplayPolicies {
                colour,
                emoji,
                progress,
                accessibility,
                json,
            },
            facts: TerminalFacts::from_signals(
                EnvSignals {
                    no_color_set,
                    term_dumb,
                    redraw_capable,
                },
                stdout_is_terminal,
                stderr_is_terminal,
            ),
        });
    }

    sweep.assert_complete();
}

#[derive(Default)]
struct DomainSweep {
    cases_seen: usize,
    truth_mismatches: usize,
    progress_mismatches: usize,
    mismatch_examples: Vec<String>,
    coverage: Coverage,
}

impl DomainSweep {
    /// Compare one domain point with the independent expectation and record coverage.
    fn observe(&mut self, case: DisplayCase) {
        let plan = resolve_display_plan(case.policies, case.facts);
        let expected = expected_display(case);
        let actual = actual_display(plan);
        if actual != expected {
            self.truth_mismatches += 1;
            if self.mismatch_examples.len() < 3 {
                self.mismatch_examples
                    .push(format!("{case:?}: got {actual:?}, expected {expected:?}"));
            }
        }

        if matches!(case.policies.progress, ProgressPolicy::Auto) {
            self.record_auto_progress(case, plan);
        }

        self.coverage.observe(case, plan);
        self.cases_seen += 1;
    }

    /// Compare an automatic progress policy with the always policy.
    fn record_auto_progress(
        &mut self,
        case: DisplayCase,
        plan: netsuke::display_plan::DisplayPlan,
    ) {
        let always_progress = DisplayPolicies {
            progress: ProgressPolicy::Always,
            ..case.policies
        };
        if plan != resolve_display_plan(always_progress, case.facts) {
            self.progress_mismatches += 1;
        }
    }

    /// Assert that the sweep covered the full input and output domains.
    fn assert_complete(self) {
        assert_eq!(self.cases_seen, DOMAIN_SIZE);
        assert_eq!(
            self.truth_mismatches, 0,
            "{} domain cases differ from the truth model: {:?}",
            self.truth_mismatches, self.mismatch_examples
        );
        assert_eq!(
            self.progress_mismatches, 0,
            "{} cases differ between auto and always progress",
            self.progress_mismatches
        );
        self.coverage.assert_complete();
    }
}

/// Read the plan's observable projections into the independent model's shape.
fn actual_display(
    plan: netsuke::display_plan::DisplayPlan,
) -> super::display_policy_truth::ExpectedDisplay {
    super::display_policy_truth::ExpectedDisplay {
        output_mode: plan.output_mode(),
        emoji_allowed: plan.prefs().emoji_allowed(),
        reporter: plan.reporter(),
        progress: super::display_policy_truth::ExpectedProgress {
            enabled: plan.progress_enabled(),
            live: plan.live_progress(),
            force_text_task_updates: plan.force_text_task_updates(),
        },
        stderr_colour: plan.stderr_colour(),
        diagnostic_style: plan.diagnostic_style(),
    }
}

#[derive(Default)]
struct Coverage {
    emoji_policies: u8,
    colour_policies: u8,
    progress_policies: u8,
    accessibility_policies: u8,
    output_modes: u8,
    emoji_values: u8,
    reporters: u8,
    progress_values: u8,
    live_values: u8,
    text_values: u8,
    stderr_colour: u8,
    diagnostic_colour: u8,
    diagnostic_unicode: u8,
    links: u8,
}

impl Coverage {
    /// Record policy inputs and plan outputs reached by one domain point.
    fn observe(&mut self, case: DisplayCase, plan: netsuke::display_plan::DisplayPlan) {
        record(
            &mut self.emoji_policies,
            emoji_policy_index(case.policies.emoji),
        );
        record(
            &mut self.colour_policies,
            colour_policy_index(case.policies.colour),
        );
        record(
            &mut self.progress_policies,
            progress_policy_index(case.policies.progress),
        );
        record(
            &mut self.accessibility_policies,
            accessibility_policy_index(case.policies.accessibility),
        );
        record(
            &mut self.output_modes,
            output_mode_index(plan.output_mode()),
        );
        record_bool(&mut self.emoji_values, plan.prefs().emoji_allowed());
        record(&mut self.reporters, reporter_index(plan.reporter()));
        record_bool(&mut self.progress_values, plan.progress_enabled());
        record_bool(&mut self.live_values, plan.live_progress());
        record_bool(&mut self.text_values, plan.force_text_task_updates());
        record_bool(&mut self.stderr_colour, plan.stderr_colour());
        record_bool(&mut self.diagnostic_colour, plan.diagnostic_style().colour);
        record_bool(
            &mut self.diagnostic_unicode,
            plan.diagnostic_style().unicode,
        );
        record_bool(&mut self.links, plan.diagnostic_style().links);
    }

    /// Assert that the enumeration reached every policy and plan value.
    fn assert_complete(self) {
        assert_domain_covered(self.emoji_policies, 3);
        assert_domain_covered(self.colour_policies, 3);
        assert_domain_covered(self.progress_policies, 3);
        assert_domain_covered(self.accessibility_policies, 3);
        assert_domain_covered(self.output_modes, 2);
        assert_domain_covered(self.emoji_values, 2);
        assert_domain_covered(self.reporters, 3);
        assert_domain_covered(self.progress_values, 2);
        assert_domain_covered(self.live_values, 2);
        assert_domain_covered(self.text_values, 2);
        assert_domain_covered(self.stderr_colour, 2);
        assert_domain_covered(self.diagnostic_colour, 2);
        assert_domain_covered(self.diagnostic_unicode, 2);
        assert_domain_covered(self.links, 2);
    }
}

/// Mark an enum value in a compact bitset without indexing a collection.
fn record(coverage: &mut u8, value: u8) {
    *coverage |= 1 << value;
}

/// Mark a boolean value in a compact bitset.
fn record_bool(coverage: &mut u8, value: bool) {
    record(coverage, u8::from(value));
}

/// Assert that all expected low-order bits were observed.
fn assert_domain_covered(coverage: u8, value_count: u8) {
    assert_eq!(coverage, (1 << value_count) - 1);
}

/// Map an emoji policy to its coverage bit.
const fn emoji_policy_index(policy: EmojiPolicy) -> u8 {
    match policy {
        EmojiPolicy::Auto => 0,
        EmojiPolicy::Always => 1,
        EmojiPolicy::Never => 2,
    }
}

/// Map a colour policy to its coverage bit.
const fn colour_policy_index(policy: ColourPolicy) -> u8 {
    match policy {
        ColourPolicy::Auto => 0,
        ColourPolicy::Always => 1,
        ColourPolicy::Never => 2,
    }
}

/// Map a progress policy to its coverage bit.
const fn progress_policy_index(policy: ProgressPolicy) -> u8 {
    match policy {
        ProgressPolicy::Auto => 0,
        ProgressPolicy::Always => 1,
        ProgressPolicy::Never => 2,
    }
}

/// Map an accessibility policy to its coverage bit.
const fn accessibility_policy_index(policy: AccessibilityPolicy) -> u8 {
    match policy {
        AccessibilityPolicy::Auto => 0,
        AccessibilityPolicy::On => 1,
        AccessibilityPolicy::Off => 2,
    }
}

/// Map an output mode to its coverage bit.
const fn output_mode_index(mode: OutputMode) -> u8 {
    match mode {
        OutputMode::Accessible => 0,
        OutputMode::Standard => 1,
    }
}

/// Map a reporter selection to its coverage bit.
const fn reporter_index(reporter: ReporterKind) -> u8 {
    match reporter {
        ReporterKind::Silent => 0,
        ReporterKind::Accessible => 1,
        ReporterKind::Standard => 2,
    }
}
