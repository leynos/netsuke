//! CLI localization helpers.
//!
//! This module keeps clap localization logic separate from the core CLI
//! definitions.

use clap::Command;
use ortho_config::{LocalizationArgs, Localizer};
use std::ffi::OsString;

use crate::localization::keys;

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
        let mut updated = std::mem::take(subcommand);
        if let Some(localized) = localize_field(
            localizer,
            known.map(subcommand_about_key),
            updated
                .get_about()
                .map(|s: &clap::builder::StyledStr| s.to_string()),
        ) {
            updated = updated.about(localized);
        }

        if let Some(localized) = localize_field(
            localizer,
            known.map(subcommand_long_about_key),
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

/// The set of known CLI subcommands.
///
/// Replaces raw `&str` subcommand-name parameters in localization helpers to
/// eliminate primitive obsession.
#[derive(Clone, Copy)]
enum Subcommand {
    /// The `build` subcommand.
    Build,
    /// The `check` subcommand.
    Check,
    /// The `clean` subcommand.
    Clean,
    /// The `graph` subcommand.
    Graph,
    /// The `generate` subcommand.
    Generate,
    /// The `help` subcommand.
    Help,
}

impl Subcommand {
    /// Resolve a subcommand from its CLI name.
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "build" => Some(Self::Build),
            "check" => Some(Self::Check),
            "clean" => Some(Self::Clean),
            "graph" => Some(Self::Graph),
            "generate" => Some(Self::Generate),
            "help" => Some(Self::Help),
            _ => None,
        }
    }
}

/// The topics nested under the `help` subcommand.
#[derive(Clone, Copy)]
enum HelpTopicName {
    /// The `targets` help topic.
    Targets,
    /// A help topic describing a known subcommand.
    Subcommand(Subcommand),
}

impl HelpTopicName {
    /// Resolve a help topic from its CLI name.
    fn from_name(name: &str) -> Option<Self> {
        if name == "targets" {
            return Some(Self::Targets);
        }

        Subcommand::from_name(name).and_then(|subcommand| match subcommand {
            Subcommand::Build
            | Subcommand::Check
            | Subcommand::Clean
            | Subcommand::Graph
            | Subcommand::Generate => Some(Self::Subcommand(subcommand)),
            Subcommand::Help => None,
        })
    }
}

/// Return the help key for a flag within a subcommand, when one is known.
fn flag_help_key(arg_id: &str, subcommand: Option<Subcommand>) -> Option<&'static str> {
    match subcommand {
        None => top_level_flag_help_key(arg_id),
        Some(Subcommand::Build) => build_flag_help_key(arg_id),
        Some(Subcommand::Check) => check_flag_help_key(arg_id),
        Some(Subcommand::Graph) => graph_flag_help_key(arg_id),
        Some(Subcommand::Generate) => generate_flag_help_key(arg_id),
        Some(Subcommand::Clean | Subcommand::Help) => None,
    }
}

/// Return the help key for a top-level flag, when one is known.
pub(crate) fn top_level_flag_help_key(arg_id: &str) -> Option<&'static str> {
    match arg_id {
        "file" => Some(keys::CLI_FLAG_FILE_HELP),
        "directory" => Some(keys::CLI_FLAG_DIRECTORY_HELP),
        "config" => Some(keys::CLI_FLAG_CONFIG_HELP),
        "jobs" => Some(keys::CLI_FLAG_JOBS_HELP),
        "verbose" => Some(keys::CLI_FLAG_VERBOSE_HELP),
        "locale" => Some(keys::CLI_FLAG_LOCALE_HELP),
        "fetch_allow_scheme" => Some(keys::CLI_FLAG_FETCH_ALLOW_SCHEME_HELP),
        "fetch_allow_host" => Some(keys::CLI_FLAG_FETCH_ALLOW_HOST_HELP),
        "fetch_block_host" => Some(keys::CLI_FLAG_FETCH_BLOCK_HOST_HELP),
        "fetch_default_deny" => Some(keys::CLI_FLAG_FETCH_DEFAULT_DENY_HELP),
        "json" => Some(keys::CLI_FLAG_JSON_HELP),
        "no_input" => Some(keys::CLI_FLAG_NO_INPUT_HELP),
        "color" => Some(keys::CLI_FLAG_COLOR_HELP),
        "emoji" => Some(keys::CLI_FLAG_EMOJI_HELP),
        "progress" => Some(keys::CLI_FLAG_PROGRESS_HELP),
        "accessibility" => Some(keys::CLI_FLAG_ACCESSIBILITY_HELP),
        "default_targets" => Some(keys::CLI_FLAG_DEFAULT_TARGETS_HELP),
        _ => None,
    }
}

/// Return the help key for a `build` subcommand flag, when one is known.
fn build_flag_help_key(arg_id: &str) -> Option<&'static str> {
    match arg_id {
        "targets" => Some(keys::CLI_SUBCOMMAND_BUILD_FLAG_TARGETS_HELP),
        _ => None,
    }
}

/// Return the help key for a `check` subcommand flag, when one is known.
fn check_flag_help_key(arg_id: &str) -> Option<&'static str> {
    match arg_id {
        "rule" => Some(keys::CLI_SUBCOMMAND_CHECK_FLAG_RULE_HELP),
        "fail_on" => Some(keys::CLI_SUBCOMMAND_CHECK_FLAG_FAIL_ON_HELP),
        "limit" => Some(keys::CLI_SUBCOMMAND_CHECK_FLAG_LIMIT_HELP),
        "explain" => Some(keys::CLI_SUBCOMMAND_CHECK_FLAG_EXPLAIN_HELP),
        _ => None,
    }
}

/// Return the help key for a `graph` subcommand flag, when one is known.
fn graph_flag_help_key(arg_id: &str) -> Option<&'static str> {
    match arg_id {
        "html" => Some(keys::CLI_SUBCOMMAND_GRAPH_FLAG_HTML_HELP),
        "output" => Some(keys::CLI_SUBCOMMAND_GRAPH_FLAG_OUTPUT_HELP),
        _ => None,
    }
}

/// Return the help key for a `generate` subcommand flag, when one is known.
fn generate_flag_help_key(arg_id: &str) -> Option<&'static str> {
    match arg_id {
        "output" => Some(keys::CLI_SUBCOMMAND_GENERATE_FLAG_OUTPUT_HELP),
        _ => None,
    }
}

/// Return the localization key for a subcommand's short about text.
const fn subcommand_about_key(subcommand: Subcommand) -> &'static str {
    match subcommand {
        Subcommand::Build => keys::CLI_SUBCOMMAND_BUILD_ABOUT,
        Subcommand::Check => keys::CLI_SUBCOMMAND_CHECK_ABOUT,
        Subcommand::Clean => keys::CLI_SUBCOMMAND_CLEAN_ABOUT,
        Subcommand::Graph => keys::CLI_SUBCOMMAND_GRAPH_ABOUT,
        Subcommand::Generate => keys::CLI_SUBCOMMAND_GENERATE_ABOUT,
        Subcommand::Help => keys::CLI_SUBCOMMAND_HELP_ABOUT,
    }
}

/// Return the localization key for a subcommand's long about text.
const fn subcommand_long_about_key(subcommand: Subcommand) -> &'static str {
    match subcommand {
        Subcommand::Build => keys::CLI_SUBCOMMAND_BUILD_LONG_ABOUT,
        Subcommand::Check => keys::CLI_SUBCOMMAND_CHECK_LONG_ABOUT,
        Subcommand::Clean => keys::CLI_SUBCOMMAND_CLEAN_LONG_ABOUT,
        Subcommand::Graph => keys::CLI_SUBCOMMAND_GRAPH_LONG_ABOUT,
        Subcommand::Generate => keys::CLI_SUBCOMMAND_GENERATE_LONG_ABOUT,
        Subcommand::Help => keys::CLI_SUBCOMMAND_HELP_LONG_ABOUT,
    }
}

/// Return the localization key for a help topic's about text.
const fn help_topic_about_key(topic: HelpTopicName) -> &'static str {
    match topic {
        HelpTopicName::Targets => keys::CLI_HELP_TARGETS_ABOUT,
        HelpTopicName::Subcommand(subcommand) => subcommand_about_key(subcommand),
    }
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
}
