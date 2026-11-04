//! Unit tests for fact construction and display-plan policy projections.

use super::*;
use rstest::rstest;

#[rstest]
#[case(false, false)]
#[case(false, true)]
#[case(true, false)]
#[case(true, true)]
fn from_signals_preserves_each_stream_fact(
    #[case] stdout_is_terminal: bool,
    #[case] stderr_is_terminal: bool,
) {
    let signals = EnvSignals {
        no_color_set: true,
        term_dumb: false,
        redraw_capable: true,
    };
    let facts = TerminalFacts::from_signals(signals, stdout_is_terminal, stderr_is_terminal);

    assert_eq!(
        facts,
        TerminalFacts {
            env: signals,
            stdout_is_terminal,
            stderr_is_terminal,
        }
    );
}

#[rstest]
#[case(ColourPolicy::Auto, true, EnvSignals::default(), true)]
#[case(ColourPolicy::Auto, false, EnvSignals::default(), false)]
#[case(ColourPolicy::Auto, true, EnvSignals { no_color_set: true, ..EnvSignals::default() }, false)]
#[case(ColourPolicy::Auto, true, EnvSignals { term_dumb: true, ..EnvSignals::default() }, false)]
#[case(ColourPolicy::Always, false, EnvSignals::default(), true)]
#[case(ColourPolicy::Never, true, EnvSignals::default(), false)]
fn startup_colour_respects_policy_and_stdout_facts(
    #[case] policy: ColourPolicy,
    #[case] stdout_is_terminal: bool,
    #[case] env: EnvSignals,
    #[case] expected: bool,
) {
    let facts = TerminalFacts::from_signals(env, stdout_is_terminal, false);

    assert_eq!(startup_colour(policy, facts), expected);
}

#[rstest]
#[case(ProgressPolicy::Auto, false, ReporterKind::Standard)]
#[case(ProgressPolicy::Always, false, ReporterKind::Standard)]
#[case(ProgressPolicy::Never, false, ReporterKind::Silent)]
#[case(ProgressPolicy::Always, true, ReporterKind::Silent)]
fn reporter_kind_obeys_progress_and_json_policies(
    #[case] progress: ProgressPolicy,
    #[case] json: bool,
    #[case] expected: ReporterKind,
) {
    let policies = DisplayPolicies {
        colour: ColourPolicy::Auto,
        emoji: EmojiPolicy::Auto,
        progress,
        accessibility: AccessibilityPolicy::Off,
        json,
    };

    assert_eq!(reporter_kind(policies, OutputMode::Standard), expected);
}
