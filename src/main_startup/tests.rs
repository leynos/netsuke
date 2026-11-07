//! Unit tests for the pre-configuration startup window.
//!
//! These pin the two decisions this module owns: which arguments count as a
//! request for clap's own renderer, and what colour that renderer is given. The
//! rest of the startup path is exercised end to end by `src/tests.rs` and
//! `tests/startup_diagnostics_tests.rs`.

use super::*;
use clap::Parser;
use rstest::rstest;

/// An environment reporting nothing, leaving argv to decide.
struct EmptyEnv;

impl locale_resolution::LocaleEnvProvider for EmptyEnv {
    fn var(&self, _key: &str) -> Option<String> {
        None
    }
}

/// An environment asking for colour, as a shell export would.
struct ColourAlwaysEnv;

impl locale_resolution::LocaleEnvProvider for ColourAlwaysEnv {
    fn var(&self, key: &str) -> Option<String> {
        (key == locale_resolution::NETSUKE_COLOR_ENV).then(|| String::from("always"))
    }
}

/// An environment reporting a dumb terminal, which suppresses auto colour.
struct DumbTerm;

impl locale_resolution::LocaleEnvProvider for DumbTerm {
    fn var(&self, key: &str) -> Option<String> {
        (key == "TERM").then(|| String::from("dumb"))
    }
}

/// Collect argv from string literals, keeping the program name.
fn argv(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

/// Only the four clap-rendered switches count as a renderer request.
///
/// This decision reaches clap's colour choice, so a false negative leaves
/// `--help` uncoloured on a terminal and a false positive colours a run that
/// never renders help at all.
#[rstest]
#[case(&["netsuke", "--help"], true)]
#[case(&["netsuke", "-h"], true)]
#[case(&["netsuke", "--version"], true)]
#[case(&["netsuke", "-V"], true)]
#[case(&["netsuke", "build", "--help"], true)]
// Anything after `--` is a target or recipe argument, not a Netsuke switch.
#[case(&["netsuke", "--", "--help"], false)]
#[case(&["netsuke", "generate"], false)]
#[case(&["netsuke"], false)]
fn only_clap_rendered_switches_request_help(#[case] args: &[&str], #[case] expected: bool) {
    assert_eq!(requests_help_or_version(&argv(args)), expected);
}

/// The help colour follows the resolved policy, not merely the switch.
///
/// A help request is necessary but not sufficient: `--color never`, a piped
/// stdout, and a dumb terminal each suppress the styles clap would otherwise
/// embed, while `--color always` overrides a pipe.
#[rstest]
#[case(&["netsuke", "--help"], false, true, ColorChoice::Always)]
// A piped stdout resolves auto colour to off.
#[case(&["netsuke", "--help"], false, false, ColorChoice::Never)]
// No help request means clap never renders, so the choice stays plain.
#[case(&["netsuke", "generate"], false, true, ColorChoice::Never)]
#[case(
    &["netsuke", "--color", "never", "--help"],
    false,
    true,
    ColorChoice::Never
)]
#[case(
    &["netsuke", "--color", "always", "--help"],
    false,
    false,
    ColorChoice::Always
)]
// `TERM=dumb` suppresses auto colour on an otherwise capable terminal.
#[case(&["netsuke", "--help"], true, true, ColorChoice::Never)]
fn help_colour_follows_the_resolved_policy(
    #[case] args: &[&str],
    #[case] is_dumb_terminal: bool,
    #[case] stdout_is_terminal: bool,
    #[case] expected: ColorChoice,
) {
    let signals = if is_dumb_terminal {
        StartupSignals::resolve(&argv(args), &DumbTerm, stdout_is_terminal, true)
    } else {
        StartupSignals::resolve(&argv(args), &EmptyEnv, stdout_is_terminal, true)
    };
    assert_eq!(signals.parser_color, expected);
}

/// The JSON hint is read from argv before parsing.
///
/// It is a hint, not a decision — configuration can still turn JSON on — but a
/// false negative here lets startup diagnostics reach stderr, which is where
/// the JSON document goes.
#[rstest]
#[case(&["netsuke", "--help"], false)]
#[case(&["netsuke", "--json", "generate"], true)]
fn the_json_hint_comes_from_argv(#[case] args: &[&str], #[case] expected: bool) {
    let signals = StartupSignals::resolve(&argv(args), &EmptyEnv, false, false);
    assert_eq!(signals.json_hint, expected);
}

/// The pre-configuration window applies the same colour precedence as the merge.
///
/// `--color` wins over `NETSUKE_COLOR`, and `NETSUKE_COLOR` wins over the
/// default. Parsed argv alone cannot express this: an absent `--color` parses
/// as `Auto`, exactly like an explicit `--color auto`. The runner-rendered help
/// topics never reach the configuration merge, so if this window did not fold
/// the environment value in, `NETSUKE_COLOR=always` would style clap's own
/// `--help` and leave `help` and `help build` plain.
#[rstest]
#[case(&["netsuke", "help"], false, cli::ColourPolicy::Auto)]
#[case(&["netsuke", "help"], true, cli::ColourPolicy::Always)]
#[case(&["netsuke", "--color", "never", "help"], true, cli::ColourPolicy::Never)]
#[case(&["netsuke", "--color", "always", "help"], false, cli::ColourPolicy::Always)]
fn informational_help_takes_the_startup_colour_hint(
    #[case] args: &[&str],
    #[case] is_colour_always_env: bool,
    #[case] expected: cli::ColourPolicy,
) {
    let raw = argv(args);
    let signals = if is_colour_always_env {
        StartupSignals::resolve(&raw, &ColourAlwaysEnv, true, true)
    } else {
        StartupSignals::resolve(&raw, &EmptyEnv, true, true)
    };
    let parsed = cli::Cli::try_parse_from(raw.iter()).expect("argv should parse");
    assert_eq!(
        signals.policies_before_configuration(&parsed).colour,
        expected
    );
}
