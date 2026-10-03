//! Keep Clap localization logic separate from core CLI definitions.

use crate::localization::keys;
use clap::Command;
use ortho_config::{LocalizationArgs, Localizer};
use std::ffi::OsString;

mod flag_keys;
pub(crate) use flag_keys::top_level_flag_help_key;
use flag_keys::{
    build_flag_help_key, check_flag_help_key, generate_flag_help_key, graph_flag_help_key,
};

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

/// The pair of localization keys describing one subcommand.
///
/// The two keys are looked up together and are only ever correct together, so
/// pairing them keeps one exhaustive match over [`Subcommand`] instead of two
/// that could drift apart when a subcommand is added.
#[derive(Clone, Copy)]
struct SubcommandAboutKeys {
    /// Key for the one-line summary shown in a command list.
    short: &'static str,
    /// Key for the expanded description shown by `--help`.
    long: &'static str,
}

/// Return the localization keys for a subcommand's about text.
const fn subcommand_about_keys(subcommand: Subcommand) -> SubcommandAboutKeys {
    match subcommand {
        Subcommand::Build => SubcommandAboutKeys {
            short: keys::CLI_SUBCOMMAND_BUILD_ABOUT,
            long: keys::CLI_SUBCOMMAND_BUILD_LONG_ABOUT,
        },
        Subcommand::Check => SubcommandAboutKeys {
            short: keys::CLI_SUBCOMMAND_CHECK_ABOUT,
            long: keys::CLI_SUBCOMMAND_CHECK_LONG_ABOUT,
        },
        Subcommand::Clean => SubcommandAboutKeys {
            short: keys::CLI_SUBCOMMAND_CLEAN_ABOUT,
            long: keys::CLI_SUBCOMMAND_CLEAN_LONG_ABOUT,
        },
        Subcommand::Graph => SubcommandAboutKeys {
            short: keys::CLI_SUBCOMMAND_GRAPH_ABOUT,
            long: keys::CLI_SUBCOMMAND_GRAPH_LONG_ABOUT,
        },
        Subcommand::Generate => SubcommandAboutKeys {
            short: keys::CLI_SUBCOMMAND_GENERATE_ABOUT,
            long: keys::CLI_SUBCOMMAND_GENERATE_LONG_ABOUT,
        },
        Subcommand::Help => SubcommandAboutKeys {
            short: keys::CLI_SUBCOMMAND_HELP_ABOUT,
            long: keys::CLI_SUBCOMMAND_HELP_LONG_ABOUT,
        },
    }
}

/// Return the localization key for a help topic's about text.
const fn help_topic_about_key(topic: HelpTopicName) -> &'static str {
    match topic {
        HelpTopicName::Targets => keys::CLI_HELP_TARGETS_ABOUT,
        HelpTopicName::Subcommand(subcommand) => subcommand_about_keys(subcommand).short,
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
mod tests;
