//! Routing from Clap identifiers to localization keys.
//!
//! Every subcommand, help topic, and flag maps to one Fluent key, and the
//! mapping is exhaustive by construction: a new subcommand or flag will not
//! compile until it is named here. Keeping the tables in their own module
//! separates that bookkeeping from the tree-walking in [`super`], which is
//! about how the localized text is applied rather than which text applies.

use crate::localization::keys;

/// The set of known CLI subcommands.
///
/// Replaces raw `&str` subcommand-name parameters in localization helpers to
/// eliminate primitive obsession.
#[derive(Clone, Copy)]
pub(super) enum Subcommand {
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
    pub(super) fn from_name(name: &str) -> Option<Self> {
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
pub(super) enum HelpTopicName {
    /// The `targets` help topic.
    Targets,
    /// A help topic describing a known subcommand.
    Subcommand(Subcommand),
}

impl HelpTopicName {
    /// Resolve a help topic from its CLI name.
    pub(super) fn from_name(name: &str) -> Option<Self> {
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
pub(super) fn flag_help_key(arg_id: &str, subcommand: Option<Subcommand>) -> Option<&'static str> {
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

/// The pair of localization keys describing one subcommand.
///
/// The two keys are looked up together and are only ever correct together, so
/// pairing them keeps one exhaustive match over [`Subcommand`] instead of two
/// that could drift apart when a subcommand is added.
#[derive(Clone, Copy)]
pub(super) struct SubcommandAboutKeys {
    /// Key for the one-line summary shown in a command list.
    pub(super) short: &'static str,
    /// Key for the expanded description shown by `--help`.
    pub(super) long: &'static str,
}

/// Return the localization keys for a subcommand's about text.
pub(super) const fn subcommand_about_keys(subcommand: Subcommand) -> SubcommandAboutKeys {
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
pub(super) const fn help_topic_about_key(topic: HelpTopicName) -> &'static str {
    match topic {
        HelpTopicName::Targets => keys::CLI_HELP_TARGETS_ABOUT,
        HelpTopicName::Subcommand(subcommand) => subcommand_about_keys(subcommand).short,
    }
}
