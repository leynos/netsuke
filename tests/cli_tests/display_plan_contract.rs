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
        rows_for_policy([
            (
                "auto",
                DisplayPolicies {
                    colour: ColourPolicy::Auto,
                    ..policies()
                },
            ),
            (
                "always",
                DisplayPolicies {
                    colour: ColourPolicy::Always,
                    ..policies()
                },
            ),
            (
                "never",
                DisplayPolicies {
                    colour: ColourPolicy::Never,
                    ..policies()
                },
            ),
        ])
    );
}

#[test]
fn display_plan_contract_emoji() {
    assert_yaml_snapshot!(
        "display_plan_contract_emoji",
        rows_for_policy([
            (
                "auto",
                DisplayPolicies {
                    emoji: EmojiPolicy::Auto,
                    ..policies()
                },
            ),
            (
                "always",
                DisplayPolicies {
                    emoji: EmojiPolicy::Always,
                    ..policies()
                },
            ),
            (
                "never",
                DisplayPolicies {
                    emoji: EmojiPolicy::Never,
                    ..policies()
                },
            ),
        ])
    );
}

#[test]
fn display_plan_contract_progress() {
    assert_yaml_snapshot!(
        "display_plan_contract_progress",
        rows_for_policy([
            (
                "auto",
                DisplayPolicies {
                    progress: ProgressPolicy::Auto,
                    ..policies()
                },
            ),
            (
                "always",
                DisplayPolicies {
                    progress: ProgressPolicy::Always,
                    ..policies()
                },
            ),
            (
                "never",
                DisplayPolicies {
                    progress: ProgressPolicy::Never,
                    ..policies()
                },
            ),
        ])
    );
}

#[test]
fn display_plan_contract_accessibility() {
    assert_yaml_snapshot!(
        "display_plan_contract_accessibility",
        rows_for_policy([
            (
                "auto",
                DisplayPolicies {
                    accessibility: AccessibilityPolicy::Auto,
                    ..policies()
                },
            ),
            (
                "on",
                DisplayPolicies {
                    accessibility: AccessibilityPolicy::On,
                    ..policies()
                },
            ),
            (
                "off",
                DisplayPolicies {
                    accessibility: AccessibilityPolicy::Off,
                    ..policies()
                },
            ),
        ])
    );
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
        stderr_colour: plan.stderr_colour(),
        diagnostics: DiagnosticProjection {
            colour: plan.diagnostic_style().colour,
            unicode: plan.diagnostic_style().unicode,
            links: plan.diagnostic_style().links,
        },
    }
}
