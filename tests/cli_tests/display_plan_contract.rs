//! Snapshot the projected display plan for each terminal policy.

use insta::assert_yaml_snapshot;
use netsuke::cli::config::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
use netsuke::display_plan::{
    DisplayPlan, DisplayPolicies, ReporterKind, TerminalFacts, resolve_display_plan,
};
use netsuke::output_mode::{EnvSignals, OutputMode};
use serde::Serialize;

#[derive(Serialize)]
struct PlanProjection {
    output_mode: &'static str,
    emoji_allowed: bool,
    reporter: &'static str,
    progress: ProgressProjection,
    stdout_colour: bool,
    stderr_colour: bool,
    diagnostics: DiagnosticProjection,
}

/// The three independent progress decisions a plan resolves.
#[derive(Serialize)]
struct ProgressProjection {
    enabled: bool,
    live: bool,
    force_text_task_updates: bool,
}

/// The three independent diagnostic-style decisions a plan resolves.
#[derive(Serialize)]
struct DiagnosticProjection {
    colour: bool,
    unicode: bool,
    links: bool,
}

#[derive(Serialize)]
struct ContractRow {
    policy: &'static str,
    facts: &'static str,
    plan: PlanProjection,
}

#[test]
fn display_plan_contract_colour() {
    assert_yaml_snapshot!(
        "display_plan_contract_colour",
        rows_for_policy(colour_cases())
    );
}

#[test]
fn display_plan_contract_emoji() {
    assert_yaml_snapshot!(
        "display_plan_contract_emoji",
        rows_for_policy(emoji_cases())
    );
}

#[test]
fn display_plan_contract_progress() {
    assert_yaml_snapshot!(
        "display_plan_contract_progress",
        rows_for_policy(progress_cases())
    );
}

#[test]
fn display_plan_contract_accessibility() {
    assert_yaml_snapshot!(
        "display_plan_contract_accessibility",
        rows_for_policy(accessibility_cases())
    );
}

/// Build the colour policy rows, one per accepted value.
fn colour_cases() -> [(&'static str, DisplayPolicies); 3] {
    [
        ("auto", with_colour(ColourPolicy::Auto)),
        ("always", with_colour(ColourPolicy::Always)),
        ("never", with_colour(ColourPolicy::Never)),
    ]
}

/// Build the emoji policy rows, one per accepted value.
fn emoji_cases() -> [(&'static str, DisplayPolicies); 3] {
    [
        ("auto", with_emoji(EmojiPolicy::Auto)),
        ("always", with_emoji(EmojiPolicy::Always)),
        ("never", with_emoji(EmojiPolicy::Never)),
    ]
}

/// Build the progress policy rows, one per accepted value.
fn progress_cases() -> [(&'static str, DisplayPolicies); 3] {
    [
        ("auto", with_progress(ProgressPolicy::Auto)),
        ("always", with_progress(ProgressPolicy::Always)),
        ("never", with_progress(ProgressPolicy::Never)),
    ]
}

/// Build the accessibility policy rows, one per accepted value.
fn accessibility_cases() -> [(&'static str, DisplayPolicies); 3] {
    [
        ("auto", with_accessibility(AccessibilityPolicy::Auto)),
        ("on", with_accessibility(AccessibilityPolicy::On)),
        ("off", with_accessibility(AccessibilityPolicy::Off)),
    ]
}

/// Return the baseline policies with only the colour replaced.
const fn with_colour(colour: ColourPolicy) -> DisplayPolicies {
    DisplayPolicies {
        colour,
        ..policies()
    }
}

/// Return the baseline policies with only the emoji setting replaced.
const fn with_emoji(emoji: EmojiPolicy) -> DisplayPolicies {
    DisplayPolicies {
        emoji,
        ..policies()
    }
}

/// Return the baseline policies with only the progress setting replaced.
const fn with_progress(progress: ProgressPolicy) -> DisplayPolicies {
    DisplayPolicies {
        progress,
        ..policies()
    }
}

/// Return the baseline policies with only the accessibility setting replaced.
const fn with_accessibility(accessibility: AccessibilityPolicy) -> DisplayPolicies {
    DisplayPolicies {
        accessibility,
        ..policies()
    }
}

/// Build deterministic projections for each policy against representative facts.
fn rows_for_policy<const N: usize>(
    cases: [(&'static str, DisplayPolicies); N],
) -> Vec<ContractRow> {
    cases
        .into_iter()
        .flat_map(|(policy, mut policies)| {
            fact_cases()
                .into_iter()
                .map(move |(facts_name, facts, json)| {
                    policies.json = json;
                    let plan = resolve_display_plan(policies, facts);
                    ContractRow {
                        policy,
                        facts: facts_name,
                        plan: project(plan),
                    }
                })
        })
        .collect()
}

/// Build the environment signal set a case needs.
const fn signals(no_color_set: bool, term_dumb: bool, redraw_capable: bool) -> EnvSignals {
    EnvSignals {
        no_color_set,
        term_dumb,
        redraw_capable,
    }
}

/// Build stream and environment fact combinations for each plan snapshot.
///
/// Each row names the facts it means, so a snapshot that changes says which
/// terminal shape changed rather than only which line moved.
fn fact_cases() -> [(&'static str, TerminalFacts, bool); 6] {
    let piped = TerminalFacts::from_signals(signals(false, false, false), false, false);
    [
        ("pipe", piped, false),
        (
            "stdout terminal",
            TerminalFacts::from_signals(signals(false, false, false), true, false),
            false,
        ),
        (
            "stderr terminal",
            TerminalFacts::from_signals(signals(false, false, true), false, true),
            false,
        ),
        (
            "terminal with NO_COLOR",
            TerminalFacts::from_signals(signals(true, false, true), true, true),
            false,
        ),
        (
            "dumb terminal",
            TerminalFacts::from_signals(signals(false, true, false), true, true),
            false,
        ),
        ("JSON pipe", piped, true),
    ]
}

/// Return the baseline policies used by each one-policy snapshot.
const fn policies() -> DisplayPolicies {
    DisplayPolicies {
        colour: ColourPolicy::Auto,
        emoji: EmojiPolicy::Auto,
        progress: ProgressPolicy::Auto,
        accessibility: AccessibilityPolicy::Auto,
        json: false,
    }
}

/// Project every user-visible decision from one resolved plan.
fn project(plan: DisplayPlan) -> PlanProjection {
    PlanProjection {
        output_mode: match plan.output_mode() {
            OutputMode::Standard => "standard",
            OutputMode::Accessible => "accessible",
        },
        emoji_allowed: plan.prefs().emoji_allowed(),
        reporter: match plan.reporter() {
            ReporterKind::Silent => "silent",
            ReporterKind::Accessible => "accessible",
            ReporterKind::Standard => "standard",
        },
        progress: ProgressProjection {
            enabled: plan.progress_enabled(),
            live: plan.live_progress(),
            force_text_task_updates: plan.force_text_task_updates(),
        },
        stdout_colour: plan.stdout_colour(),
        stderr_colour: plan.stderr_colour(),
        diagnostics: DiagnosticProjection {
            colour: plan.diagnostic_style().colour,
            unicode: plan.diagnostic_style().unicode,
            links: plan.diagnostic_style().links,
        },
    }
}
