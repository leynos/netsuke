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

        *subcommand = updated;
    }
}

/// The set of known CLI subcommands.
///
/// Replaces raw `&str` subcommand-name parameters in localisation helpers to
/// eliminate primitive obsession.
#[derive(Clone, Copy)]
enum Subcommand {
    Build,
    Clean,
    Graph,
    Manifest,
}

impl Subcommand {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "build" => Some(Self::Build),
            "clean" => Some(Self::Clean),
            "graph" => Some(Self::Graph),
            "manifest" => Some(Self::Manifest),
            _ => None,
        }
    }
}

fn flag_help_key(arg_id: &str, subcommand: Option<Subcommand>) -> Option<&'static str> {
    match subcommand {
        None => top_level_flag_help_key(arg_id),
        Some(Subcommand::Build) => build_flag_help_key(arg_id),
        Some(Subcommand::Graph) => graph_flag_help_key(arg_id),
        Some(Subcommand::Manifest) => manifest_flag_help_key(arg_id),
        Some(Subcommand::Clean) => None,
    }
}

fn top_level_flag_help_key(arg_id: &str) -> Option<&'static str> {
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
    }
}

fn build_flag_help_key(arg_id: &str) -> Option<&'static str> {
    match arg_id {
        "emit" => Some(keys::CLI_SUBCOMMAND_BUILD_FLAG_EMIT_HELP),
        "targets" => Some(keys::CLI_SUBCOMMAND_BUILD_FLAG_TARGETS_HELP),
        _ => None,
    }
}

fn graph_flag_help_key(arg_id: &str) -> Option<&'static str> {
    match arg_id {
        "html" => Some(keys::CLI_SUBCOMMAND_GRAPH_FLAG_HTML_HELP),
        "output" => Some(keys::CLI_SUBCOMMAND_GRAPH_FLAG_OUTPUT_HELP),
        _ => None,
    }
}

fn manifest_flag_help_key(arg_id: &str) -> Option<&'static str> {
    match arg_id {
        "file" => Some(keys::CLI_SUBCOMMAND_MANIFEST_FLAG_FILE_HELP),
        _ => None,
    }
}

const fn subcommand_about_key(subcommand: Subcommand) -> &'static str {
    match subcommand {
        Subcommand::Build => keys::CLI_SUBCOMMAND_BUILD_ABOUT,
        Subcommand::Clean => keys::CLI_SUBCOMMAND_CLEAN_ABOUT,
        Subcommand::Graph => keys::CLI_SUBCOMMAND_GRAPH_ABOUT,
        Subcommand::Manifest => keys::CLI_SUBCOMMAND_MANIFEST_ABOUT,
    }
}

const fn subcommand_long_about_key(subcommand: Subcommand) -> &'static str {
    match subcommand {
        Subcommand::Build => keys::CLI_SUBCOMMAND_BUILD_LONG_ABOUT,
        Subcommand::Clean => keys::CLI_SUBCOMMAND_CLEAN_LONG_ABOUT,
        Subcommand::Graph => keys::CLI_SUBCOMMAND_GRAPH_LONG_ABOUT,
        Subcommand::Manifest => keys::CLI_SUBCOMMAND_MANIFEST_LONG_ABOUT,
    }
}

/// Raw argument hints collected before clap parsing runs.
///
/// `values` holds the last value seen for each requested value-taking flag
/// (from either `--flag value` or `--flag=value`); `flags` records which
/// requested bare flags appeared. Scanning stops at a `--` terminator.
#[derive(Debug, Default)]
struct RawArgHints {
    values: std::collections::HashMap<&'static str, String>,
    flags: std::collections::HashSet<&'static str>,
}

impl RawArgHints {
    fn value(&self, flag: &str) -> Option<&str> {
        self.values.get(flag).map(String::as_str)
    }

    fn has_flag(&self, flag: &str) -> bool {
        self.flags.contains(flag)
    }
}

/// Scan raw arguments for startup hints shared by locale and diagnostic-mode
/// detection.
///
/// The scanner implements the option grammar both hint paths must agree on:
///
/// - scanning stops at a bare `--` terminator;
/// - a flag in `value_flags` consumes the next argument as its value unless
///   that argument is `--` or absent, in which case scanning stops;
/// - `--flag=value` records the value inline;
/// - the last occurrence of a value flag wins;
/// - a flag in `bare_flags` is recorded by presence alone.
///
/// Value interpretation stays with the callers, keeping this scanner purely
/// lexical.
fn scan_raw_hints(
    args: &[OsString],
    value_flags: &[&'static str],
    bare_flags: &[&'static str],
) -> RawArgHints {
    let mut hints = RawArgHints::default();
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        let text = arg.to_string_lossy();
        if text == "--" {
            break;
        }
        if let Some(flag) = bare_flags.iter().find(|flag| text == **flag) {
            hints.flags.insert(flag);
            continue;
        }
        if scan_value_flags(&text, &mut iter, value_flags, &mut hints) == ScanStep::Stop {
            break;
        }
    }
    hints
}

/// Outcome of scanning one argument: keep going or stop at a terminator.
#[derive(Debug, PartialEq, Eq)]
enum ScanStep {
    Continue,
    Stop,
}

type RawArgIter<'a> = std::iter::Peekable<std::slice::Iter<'a, OsString>>;

/// Match `text` against the value-taking flags, recording any value found.
fn scan_value_flags(
    text: &str,
    iter: &mut RawArgIter<'_>,
    value_flags: &[&'static str],
    hints: &mut RawArgHints,
) -> ScanStep {
    for flag in value_flags {
        if text == *flag {
            return consume_flag_value(flag, iter, hints);
        }
        if let Some(value) = text
            .strip_prefix(flag)
            .and_then(|rest| rest.strip_prefix('='))
        {
            hints.values.insert(flag, value.to_owned());
            return ScanStep::Continue;
        }
    }
    ScanStep::Continue
}

/// Consume the argument following `flag` as its value.
///
/// A missing value or a `--` terminator stops the scan, mirroring the
/// behaviour of the original per-flag scanners.
fn consume_flag_value(
    flag: &'static str,
    iter: &mut RawArgIter<'_>,
    hints: &mut RawArgHints,
) -> ScanStep {
    let Some(next) = iter.peek() else {
        return ScanStep::Stop;
    };
    let next_text = next.to_string_lossy();
    if next_text == "--" {
        return ScanStep::Stop;
    }
    hints.values.insert(flag, next_text.into_owned());
    iter.next();
    ScanStep::Continue
}

/// Inspect raw arguments and extract the `--locale` value when present.
///
/// When multiple `--locale` flags are provided, the last one is used.
#[must_use]
pub fn locale_hint_from_args(args: &[OsString]) -> Option<String> {
    scan_raw_hints(args, &["--locale"], &[])
        .value("--locale")
        .map(str::to_owned)
}

pub(crate) fn parse_bool_hint(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Inspect raw arguments and detect whether JSON diagnostics were requested.
///
/// A bare `--diag-json` enables JSON diagnostics. `--output-format json` is a
/// diagnostic-format alias and `--output-format human` disables JSON
/// diagnostics. The helper mirrors clap's flag semantics, so
/// `--diag-json=value` is ignored rather than interpreted as a boolean
/// assignment.
#[must_use]
pub fn diag_json_hint_from_args(args: &[OsString]) -> Option<bool> {
    let hints = scan_raw_hints(args, &["--output-format"], &["--diag-json"]);
    let output_format_hint = hints
        .value("--output-format")
        .and_then(diag_json_hint_from_output_format);
    let diag_json_hint = hints.has_flag("--diag-json").then_some(true);
    output_format_hint.or(diag_json_hint)
}

fn diag_json_hint_from_output_format(value: &str) -> Option<bool> {
    match value {
        "json" => Some(true),
        "human" => Some(false),
        _ => None,
    }
}
