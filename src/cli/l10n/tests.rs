//! Unit tests for CLI localization helper routing.

use super::*;
use rstest::rstest;

/// Verify that help topic names map only to supported about keys.
#[rstest]
#[case("targets", Some(keys::CLI_HELP_TARGETS_ABOUT))]
#[case("build", Some(keys::CLI_SUBCOMMAND_BUILD_ABOUT))]
#[case("check", Some(keys::CLI_SUBCOMMAND_CHECK_ABOUT))]
#[case("clean", Some(keys::CLI_SUBCOMMAND_CLEAN_ABOUT))]
#[case("graph", Some(keys::CLI_SUBCOMMAND_GRAPH_ABOUT))]
#[case("generate", Some(keys::CLI_SUBCOMMAND_GENERATE_ABOUT))]
#[case("help", None)]
#[case("unknown", None)]
fn help_topic_names_map_to_supported_about_keys(
    #[case] name: &str,
    #[case] expected: Option<&str>,
) {
    assert_eq!(
        HelpTopicName::from_name(name).map(help_topic_about_key),
        expected
    );
}

/// Every subcommand maps to its own short and long about keys.
///
/// The pair is asserted together because pairing them in one lookup is
/// what this routing exists to guarantee: a subcommand that took another
/// command's long text would still pass a test that checked only the short
/// key.
#[rstest]
#[case(
    "build",
    keys::CLI_SUBCOMMAND_BUILD_ABOUT,
    keys::CLI_SUBCOMMAND_BUILD_LONG_ABOUT
)]
#[case(
    "check",
    keys::CLI_SUBCOMMAND_CHECK_ABOUT,
    keys::CLI_SUBCOMMAND_CHECK_LONG_ABOUT
)]
#[case(
    "clean",
    keys::CLI_SUBCOMMAND_CLEAN_ABOUT,
    keys::CLI_SUBCOMMAND_CLEAN_LONG_ABOUT
)]
#[case(
    "graph",
    keys::CLI_SUBCOMMAND_GRAPH_ABOUT,
    keys::CLI_SUBCOMMAND_GRAPH_LONG_ABOUT
)]
#[case(
    "generate",
    keys::CLI_SUBCOMMAND_GENERATE_ABOUT,
    keys::CLI_SUBCOMMAND_GENERATE_LONG_ABOUT
)]
#[case(
    "help",
    keys::CLI_SUBCOMMAND_HELP_ABOUT,
    keys::CLI_SUBCOMMAND_HELP_LONG_ABOUT
)]
fn subcommands_map_to_their_own_about_keys(
    #[case] name: &str,
    #[case] short: &str,
    #[case] long: &str,
) {
    let subcommand = Subcommand::from_name(name).expect("the fixture names a known subcommand");
    let about = subcommand_about_keys(subcommand);
    assert_eq!(about.short, short, "{name} short about key");
    assert_eq!(about.long, long, "{name} long about key");
}

/// Every `check` flag resolves to its own localized help key, and an
/// unrecognized identifier falls through rather than borrowing one.
///
/// The fallback case is the one worth pinning: a routing table that
/// returned some key for an unknown argument would silently mislabel any
/// flag added later without being wired up here.
#[rstest]
#[case("rule", Some(keys::CLI_SUBCOMMAND_CHECK_FLAG_RULE_HELP))]
#[case("fail_on", Some(keys::CLI_SUBCOMMAND_CHECK_FLAG_FAIL_ON_HELP))]
#[case("limit", Some(keys::CLI_SUBCOMMAND_CHECK_FLAG_LIMIT_HELP))]
#[case("explain", Some(keys::CLI_SUBCOMMAND_CHECK_FLAG_EXPLAIN_HELP))]
#[case("no_such_flag", None)]
fn check_flags_resolve_to_their_help_keys(#[case] arg_id: &str, #[case] expected: Option<&str>) {
    assert_eq!(flag_help_key(arg_id, Some(Subcommand::Check)), expected);
}

/// No two subcommands may share an about key.
///
/// A copy-and-paste slip in the routing table is otherwise invisible: two
/// commands would simply describe themselves identically, and every
/// per-command assertion above would still pass for the one that was
/// written correctly.
#[test]
fn about_keys_are_unique_across_subcommands() {
    let names = ["build", "check", "clean", "graph", "generate", "help"];
    let mut seen: Vec<&str> = Vec::new();
    for name in names {
        let subcommand = Subcommand::from_name(name).expect("the fixture names a known subcommand");
        let about = subcommand_about_keys(subcommand);
        seen.push(about.short);
        seen.push(about.long);
    }
    let mut unique = seen.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        seen.len(),
        unique.len(),
        "two subcommands share an about key: {seen:?}"
    );
}
