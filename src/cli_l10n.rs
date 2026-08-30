//! CLI localization helpers.
//!
//! This module keeps clap localization logic separate from the core CLI
//! definitions.

use clap::Command;
use ortho_config::{LocalizationArgs, Localizer};
use std::ffi::OsString;

use crate::localization::keys;

#[path = "cli_l10n_keys.rs"]
mod keys_routing;

use keys_routing::{
    HelpTopicName, Subcommand, flag_help_key, help_topic_about_key, subcommand_about_keys,
};

/// The table moved to `keys_routing`; `cli::release_help` still reaches it
/// through this module, so re-export it rather than reroute every caller.
pub(crate) use keys_routing::top_level_flag_help_key;

/// Strip the leading `Usage: ` prefix from a rendered usage string.
fn usage_body(usage: &str) -> &str {
    usage.strip_prefix("Usage: ").unwrap_or(usage)
}

/// Localize a command's usage, about text, argument help, and subcommands.
pub(crate) fn localize_command(mut command: Command, localizer: &dyn Localizer) -> Command {
    let rendered_usage = command.clone().render_usage().to_string();
    let fallback_usage = usage_body(&rendered_usage).to_owned();
    let mut args = LocalizationArgs::default();
    args.insert("binary", command.get_name().to_owned().into());
    args.insert("usage", fallback_usage.clone().into());
    let usage = localizer.message(keys::CLI_USAGE, Some(&args), &fallback_usage);
    command = command.override_usage(usage);

    if let Some(about) = command
        .get_about()
        .map(|s: &clap::builder::StyledStr| s.to_string())
    {
        let localized_text = localizer.message(keys::CLI_ABOUT, None, &about);
        command = command.about(localized_text);
    } else if let Some(message) = localizer.lookup(keys::CLI_ABOUT, None) {
        command = command.about(message);
    }

    if let Some(long_about) = command
        .get_long_about()
        .map(|s: &clap::builder::StyledStr| s.to_string())
    {
        let localized_text = localizer.message(keys::CLI_LONG_ABOUT, None, &long_about);
        command = command.long_about(localized_text);
    } else if let Some(message) = localizer.lookup(keys::CLI_LONG_ABOUT, None) {
        command = command.long_about(message);
    }

    command = localize_arguments(command, localizer, None);
    localize_subcommands(&mut command, localizer);

    command
}

/// Localize help text for all arguments in a command.
///
/// When `subcommand` is `None`, keys are looked up as `cli.flag.{arg_id}.help`.
/// When a subcommand is provided, keys are
/// `cli.subcommand.{name}.flag.{arg_id}.help`.
fn localize_arguments(
    command: Command,
    localizer: &dyn Localizer,
    subcommand: Option<Subcommand>,
) -> Command {
    command.mut_args(|arg| {
        let arg_id = arg.get_id().as_str();
        let Some(key) = flag_help_key(arg_id, subcommand) else {
            return arg;
        };
        if let Some(help) = arg
            .get_help()
            .map(|s: &clap::builder::StyledStr| s.to_string())
        {
            let message = localizer.message(key, None, &help);
            return arg.help(message);
        }
        if let Some(message) = localizer.lookup(key, None) {
            return arg.help(message);
        }
        arg
    })
}

/// Localize a single help field, returning translated text when a key exists.
fn localize_field(
    localizer: &dyn Localizer,
    key: Option<&'static str>,
    current_value: Option<String>,
) -> Option<String> {
    let key_id = key?;
    if let Some(value) = current_value {
        return Some(localizer.message(key_id, None, &value));
    }
    localizer.lookup(key_id, None)
}

/// Localize the about text, argument help, and help topics of every subcommand.
fn localize_subcommands(command: &mut Command, localizer: &dyn Localizer) {
    for subcommand in command.get_subcommands_mut() {
        let known = Subcommand::from_name(subcommand.get_name());
        // Resolve the pair once: the short and long keys are only ever correct
        // together, so looking them up separately would invite them to drift.
        let about = known.map(subcommand_about_keys);
        let mut updated = std::mem::take(subcommand);
        if let Some(localized) = localize_field(
            localizer,
            about.map(|entry| entry.short),
            updated
                .get_about()
                .map(|s: &clap::builder::StyledStr| s.to_string()),
        ) {
            updated = updated.about(localized);
        }

        if let Some(localized) = localize_field(
            localizer,
            about.map(|entry| entry.long),
            updated
                .get_long_about()
                .map(|s: &clap::builder::StyledStr| s.to_string()),
        ) {
            updated = updated.long_about(localized);
        }

        // Localise subcommand argument help text.
        updated = localize_arguments(updated, localizer, known);
        updated = localize_help_topics(updated, localizer, known);

        *subcommand = updated;
    }
}

/// Localize the topics nested beneath the `help` subcommand.
fn localize_help_topics(
    mut command: Command,
    localizer: &dyn Localizer,
    subcommand: Option<Subcommand>,
) -> Command {
    if !matches!(subcommand, Some(Subcommand::Help)) {
        return command;
    }

    for topic in command.get_subcommands_mut() {
        let known = HelpTopicName::from_name(topic.get_name());
        let mut updated = std::mem::take(topic);
        if let Some(localized) = localize_field(
            localizer,
            known.map(help_topic_about_key),
            updated
                .get_about()
                .map(|s: &clap::builder::StyledStr| s.to_string()),
        ) {
            updated = updated.about(localized);
        }
        *topic = updated;
    }

    command
}

/// Inspect raw arguments and extract the `--locale` value when present.
///
/// When multiple `--locale` flags are provided, the last one is used.
/// This valued scanner intentionally remains separate from the bare
/// `json_hint_from_args` flag scanner. Extract `find_option_value` only when a
/// second valued pre-clap option needs the same handling.
#[must_use]
pub fn locale_hint_from_args(args: &[OsString]) -> Option<String> {
    let mut hint = None;
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        let text = arg.to_string_lossy();
        if text == "--" {
            break;
        }
        if text == "--locale" {
            let Some(next) = iter.peek() else {
                break;
            };
            let next_text = next.to_string_lossy();
            if next_text == "--" {
                break;
            }
            hint = Some(next_text.into_owned());
            iter.next();
            continue;
        }
        if let Some(value) = text.strip_prefix("--locale=") {
            hint = Some(value.to_owned());
        }
    }
    hint
}

/// Parse a user-supplied boolean value, returning `None` for unrecognized input.
pub(crate) fn parse_bool_hint(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Inspect raw arguments and detect whether JSON output was requested.
///
/// The helper mirrors clap's flag semantics, so `--json=value` is ignored
/// rather than interpreted as a boolean assignment.
/// It intentionally remains separate from the valued `locale_hint_from_args`
/// scanner; extract `find_option_value` when a second valued pre-clap option
/// needs the same handling.
#[must_use]
pub fn json_hint_from_args(args: &[OsString]) -> Option<bool> {
    for arg in args {
        let text = arg.to_string_lossy();
        if text == "--" {
            break;
        }
        if text == "--json" {
            return Some(true);
        }
    }
    None
}

#[cfg(test)]
mod tests {
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
            let subcommand =
                Subcommand::from_name(name).expect("the fixture names a known subcommand");
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
}
