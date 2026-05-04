//! CLI localization helpers.
//!
//! This module keeps clap localization logic separate from the core CLI
//! definitions.

use clap::Command;
use ortho_config::{LocalizationArgs, Localizer};
use std::ffi::OsString;

use crate::localization::keys;

fn usage_body(usage: &str) -> &str {
    usage.strip_prefix("Usage: ").unwrap_or(usage)
}

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

/// Localise help text for all arguments in a command.
///
/// When `subcommand_name` is `None`, keys are looked up as `cli.flag.{arg_id}.help`.
/// When a subcommand name is provided, keys are `cli.subcommand.{name}.flag.{arg_id}.help`.
fn localize_arguments(
    command: Command,
    localizer: &dyn Localizer,
    subcommand_name: Option<&str>,
) -> Command {
    command.mut_args(|arg| {
        let arg_id = arg.get_id().as_str();
        let Some(key) = flag_help_key(arg_id, subcommand_name) else {
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

fn localize_subcommands(command: &mut Command, localizer: &dyn Localizer) {
    for subcommand in command.get_subcommands_mut() {
        let name = subcommand.get_name().to_owned();
        let mut updated = std::mem::take(subcommand);
        if let Some(localized) = localize_field(
            localizer,
            subcommand_about_key(&name),
            updated
                .get_about()
                .map(|s: &clap::builder::StyledStr| s.to_string()),
        ) {
            updated = updated.about(localized);
        }

        if let Some(localized) = localize_field(
            localizer,
            subcommand_long_about_key(&name),
            updated
                .get_long_about()
                .map(|s: &clap::builder::StyledStr| s.to_string()),
        ) {
            updated = updated.long_about(localized);
        }

        // Localise subcommand argument help text.
        updated = localize_arguments(updated, localizer, Some(&name));

        *subcommand = updated;
    }
}

fn flag_help_key(arg_id: &str, subcommand_name: Option<&str>) -> Option<&'static str> {
    match subcommand_name {
        None => match arg_id {
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
            "accessible" => Some(keys::CLI_FLAG_ACCESSIBLE_HELP),
            "progress" => Some(keys::CLI_FLAG_PROGRESS_HELP),
            "no_emoji" => Some(keys::CLI_FLAG_NO_EMOJI_HELP),
            "theme" => Some(keys::CLI_FLAG_THEME_HELP),
            "colour_policy" => Some(keys::CLI_FLAG_COLOUR_POLICY_HELP),
            "spinner_mode" => Some(keys::CLI_FLAG_SPINNER_MODE_HELP),
            "diag_json" => Some(keys::CLI_FLAG_DIAG_JSON_HELP),
            "output_format" => Some(keys::CLI_FLAG_OUTPUT_FORMAT_HELP),
            "default_targets" => Some(keys::CLI_FLAG_DEFAULT_TARGETS_HELP),
            _ => None,
        },
        Some("build") => match arg_id {
            "emit" => Some(keys::CLI_SUBCOMMAND_BUILD_FLAG_EMIT_HELP),
            "targets" => Some(keys::CLI_SUBCOMMAND_BUILD_FLAG_TARGETS_HELP),
            _ => None,
        },
        Some("manifest") => match arg_id {
            "file" => Some(keys::CLI_SUBCOMMAND_MANIFEST_FLAG_FILE_HELP),
            _ => None,
        },
        _ => None,
    }
}

fn subcommand_about_key(name: &str) -> Option<&'static str> {
    match name {
        "build" => Some(keys::CLI_SUBCOMMAND_BUILD_ABOUT),
        "clean" => Some(keys::CLI_SUBCOMMAND_CLEAN_ABOUT),
        "graph" => Some(keys::CLI_SUBCOMMAND_GRAPH_ABOUT),
        "manifest" => Some(keys::CLI_SUBCOMMAND_MANIFEST_ABOUT),
        _ => None,
    }
}

fn subcommand_long_about_key(name: &str) -> Option<&'static str> {
    match name {
        "build" => Some(keys::CLI_SUBCOMMAND_BUILD_LONG_ABOUT),
        "clean" => Some(keys::CLI_SUBCOMMAND_CLEAN_LONG_ABOUT),
        "graph" => Some(keys::CLI_SUBCOMMAND_GRAPH_LONG_ABOUT),
        "manifest" => Some(keys::CLI_SUBCOMMAND_MANIFEST_LONG_ABOUT),
        _ => None,
    }
}

/// Inspect raw arguments and extract the `--locale` value when present.
///
/// When multiple `--locale` flags are provided, the last one is used.
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

pub(crate) fn parse_bool_hint(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Inspect raw arguments and detect whether `--diag-json` was supplied.
///
/// A bare `--diag-json` enables JSON diagnostics. The helper mirrors clap's
/// flag semantics, so `--diag-json=value` is ignored rather than interpreted
/// as a boolean assignment.
#[must_use]
pub fn diag_json_hint_from_args(args: &[OsString]) -> Option<bool> {
    args.iter()
        .take_while(|arg| arg.to_string_lossy() != "--")
        .find_map(|arg| (arg.to_string_lossy() == "--diag-json").then_some(true))
}
