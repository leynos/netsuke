//! Localisation-aware parsing helpers for the CLI command schema.
//!
//! [`super::command`] owns [`Cli`] and its Clap definitions. This module
//! localises that schema, installs [`LocalizedValueParser`] instances for every
//! typed argument, and provides [`parse_with_localizer_from`] for downstream
//! processing.
//!
//! **Pipeline position:** parsing layer.
//!
//! - Receives raw `OsStr` arguments from the process entry point.
//! - Delegates value validation to [`super::parsing`] helpers.
//! - Returns a `Cli`/`ArgMatches` pair consumed by [`super::merge`].
//!
//! [`LocalizedValueParser`]: super::value_parser::LocalizedValueParser

use clap::builder::ValueParser;
use clap::{ArgMatches, CommandFactory};
use ortho_config::{LocalizationArgs, Localizer, parse_localized_command};
use std::ffi::OsString;
use std::sync::Arc;

use super::command::Cli;
use super::parsing::{
    parse_accessibility_policy, parse_color_policy, parse_emoji_policy, parse_host_pattern,
    parse_jobs, parse_locale, parse_progress_policy, parse_scheme,
};
use super::policy_values::{
    accessibility_policy_possible_values, colour_policy_possible_values,
    emoji_policy_possible_values, progress_policy_possible_values,
};
use super::value_parser::LocalizedValueParser;
use crate::cli_l10n::localize_command;
pub use crate::cli_l10n::{json_hint_from_args, locale_hint_from_args};
use crate::cli_localization::build_localizer;

/// Return the localized message for `key`, or `fallback` when no translation exists.
pub(super) fn validation_message(
    localizer: &dyn Localizer,
    key: &'static str,
    args: Option<&LocalizationArgs<'_>>,
    fallback: &str,
) -> String {
    localizer.message(key, args, fallback)
}

/// Parse CLI arguments with localized Clap output.
///
/// Returns both the parsed CLI struct and the `ArgMatches` required for
/// configuration merging.
///
/// # Errors
///
/// Returns a `clap::Error` with localization applied when parsing fails.
pub fn parse_with_localizer_from<I, T>(
    iter: I,
    localizer: &Arc<dyn Localizer>,
) -> Result<(Cli, ArgMatches), clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let command = configured_command(Some(localizer));
    parse_localized_command(command, iter, localizer.as_ref())
}

/// Construct the command with every typed validation parser installed.
///
/// A supplied localizer localizes the command; without one, the helper retains
/// source `en-US` wording for runtime help consumers.
pub(crate) fn configured_command(localizer: Option<&Arc<dyn Localizer>>) -> clap::Command {
    let parser_localizer = localizer
        .cloned()
        .unwrap_or_else(|| Arc::from(build_localizer(None)));
    let command = localizer.map_or_else(Cli::command, |active_localizer| {
        localize_command(Cli::command(), active_localizer.as_ref())
    });
    configure_validation_parsers(command, &parser_localizer)
}

/// Install localized value parsers on CLI arguments with localized validation.
fn configure_validation_parsers(
    mut command: clap::Command,
    localizer: &Arc<dyn Localizer>,
) -> clap::Command {
    let jobs_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_jobs);
    let locale_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_locale);
    let scheme_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_scheme);
    let host_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_host_pattern);
    let color_policy_parser = LocalizedValueParser::with_possible_values(
        Arc::clone(localizer),
        parse_color_policy,
        colour_policy_possible_values(),
    );
    let emoji_policy_parser = LocalizedValueParser::with_possible_values(
        Arc::clone(localizer),
        parse_emoji_policy,
        emoji_policy_possible_values(),
    );
    let progress_policy_parser = LocalizedValueParser::with_possible_values(
        Arc::clone(localizer),
        parse_progress_policy,
        progress_policy_possible_values(),
    );
    let accessibility_policy_parser = LocalizedValueParser::with_possible_values(
        Arc::clone(localizer),
        parse_accessibility_policy,
        accessibility_policy_possible_values(),
    );

    command = command.mut_arg("jobs", |arg| {
        arg.value_parser(ValueParser::new(jobs_parser))
    });
    command = command.mut_arg("locale", |arg| {
        arg.value_parser(ValueParser::new(locale_parser))
    });
    command = command.mut_arg("fetch_allow_scheme", |arg| {
        arg.value_parser(ValueParser::new(scheme_parser.clone()))
    });
    command = command.mut_arg("fetch_allow_host", |arg| {
        arg.value_parser(ValueParser::new(host_parser.clone()))
    });
    command = command.mut_arg("fetch_block_host", |arg| {
        arg.value_parser(ValueParser::new(host_parser))
    });
    command = command.mut_arg("color", |arg| {
        arg.value_parser(ValueParser::new(color_policy_parser))
    });
    command = command.mut_arg("emoji", |arg| {
        arg.value_parser(ValueParser::new(emoji_policy_parser))
    });
    command = command.mut_arg("progress", |arg| {
        arg.value_parser(ValueParser::new(progress_policy_parser))
    });
    command = command.mut_arg("accessibility", |arg| {
        arg.value_parser(ValueParser::new(accessibility_policy_parser))
    });
    command
}

#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;
