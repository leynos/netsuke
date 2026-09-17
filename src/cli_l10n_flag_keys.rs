//! Localization keys for CLI flags, keyed by Clap argument identifier.
//!
//! Keeping this lookup table separate from [`super::cli_l10n`] keeps the
//! localization module within the repository's module-size boundary. The table
//! is a flat argument-identifier-to-key mapping that grows by one row whenever
//! a flag is added, so it has no shared logic with the surrounding
//! localization flow.

use crate::localization::keys;

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
        "env_allow_var" => Some(keys::CLI_FLAG_ENV_ALLOW_VAR_HELP),
        "env_block_var" => Some(keys::CLI_FLAG_ENV_BLOCK_VAR_HELP),
        "fetch_allow_host" => Some(keys::CLI_FLAG_FETCH_ALLOW_HOST_HELP),
        "fetch_block_host" => Some(keys::CLI_FLAG_FETCH_BLOCK_HOST_HELP),
        "fetch_default_deny" => Some(keys::CLI_FLAG_FETCH_DEFAULT_DENY_HELP),
        "trust_project_fetch_policy" => Some(keys::CLI_FLAG_TRUST_PROJECT_FETCH_POLICY_HELP),

        "manifest_evaluation_fuel" => Some(keys::CLI_FLAG_MANIFEST_EVALUATION_FUEL_HELP),
        "manifest_fuel" => Some(keys::CLI_FLAG_MANIFEST_FUEL_HELP),
        "manifest_rendered_value_bytes" => Some(keys::CLI_FLAG_MANIFEST_RENDERED_VALUE_BYTES_HELP),
        "manifest_rendered_manifest_bytes" => {
            Some(keys::CLI_FLAG_MANIFEST_RENDERED_MANIFEST_BYTES_HELP)
        }
        "manifest_source_bytes" => Some(keys::CLI_FLAG_MANIFEST_SOURCE_BYTES_HELP),
        "manifest_foreach_cardinality" => Some(keys::CLI_FLAG_MANIFEST_FOREACH_CARDINALITY_HELP),
        "manifest_expanded_entries" => Some(keys::CLI_FLAG_MANIFEST_EXPANDED_ENTRIES_HELP),
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
pub(super) fn build_flag_help_key(arg_id: &str) -> Option<&'static str> {
    match arg_id {
        "targets" => Some(keys::CLI_SUBCOMMAND_BUILD_FLAG_TARGETS_HELP),
        _ => None,
    }
}

/// Return the help key for a `graph` subcommand flag, when one is known.
pub(super) fn graph_flag_help_key(arg_id: &str) -> Option<&'static str> {
    match arg_id {
        "html" => Some(keys::CLI_SUBCOMMAND_GRAPH_FLAG_HTML_HELP),
        "output" => Some(keys::CLI_SUBCOMMAND_GRAPH_FLAG_OUTPUT_HELP),
        _ => None,
    }
}

/// Return the help key for a `generate` subcommand flag, when one is known.
pub(super) fn generate_flag_help_key(arg_id: &str) -> Option<&'static str> {
    match arg_id {
        "output" => Some(keys::CLI_SUBCOMMAND_GENERATE_FLAG_OUTPUT_HELP),
        _ => None,
    }
}
