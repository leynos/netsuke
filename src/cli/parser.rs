//! Localisation-aware Clap parsing entry point.
//!
//! This module provides [`parse_with_localizer_from`], which localises the
//! Clap command declared by [`super::command`], installs localisation-aware
//! [`LocalizedValueParser`] instances for every typed argument, and returns
//! `(Cli, ArgMatches)` for downstream processing.
//!
//! **Pipeline position:** parsing layer.
//!
//! - Receives raw `OsStr` arguments from the process entry point.
//! - Delegates value validation to [`super::parsing`] helpers.
//! - Returns a `Cli`/`ArgMatches` pair consumed by [`super::merge`].

use clap::builder::{TypedValueParser, ValueParser};
use clap::error::ErrorKind;
use clap::{ArgMatches, CommandFactory, FromArgMatches};
use ortho_config::localize_clap_error_with_command;
use ortho_config::{LocalizationArgs, Localizer};
use std::ffi::OsString;
use std::sync::Arc;

use super::command::Cli;
use super::parsing::{
    parse_accessibility_policy, parse_color_policy, parse_emoji_policy, parse_host_pattern,
    parse_jobs, parse_locale, parse_progress_policy, parse_scheme,
};
use crate::cli_l10n::localize_command;
pub use crate::cli_l10n::{json_hint_from_args, locale_hint_from_args};

#[derive(Clone)]
struct LocalizedValueParser<F> {
    localizer: Arc<dyn Localizer>,
    parser: F,
}

impl<F> LocalizedValueParser<F> {
    fn new(localizer: Arc<dyn Localizer>, parser: F) -> Self {
        Self { localizer, parser }
    }
}

impl<F, T> TypedValueParser for LocalizedValueParser<F>
where
    F: Fn(&dyn Localizer, &str) -> Result<T, String> + Clone + Send + Sync + 'static,
    T: Send + Sync + Clone + 'static,
{
    type Value = T;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let mut command = cmd.clone();
        let Some(raw_value) = value.to_str() else {
            return Err(command.error(ErrorKind::InvalidUtf8, "invalid UTF-8"));
        };
        (self.parser)(self.localizer.as_ref(), raw_value)
            .map_err(|err| command.error(ErrorKind::ValueValidation, err))
    }
}

pub(super) fn validation_message(
    localizer: &dyn Localizer,
    key: &'static str,
    args: Option<&LocalizationArgs<'_>>,
    fallback: &str,
) -> String {
    localizer.message(key, args, fallback)
}

/// Parse CLI arguments with localized clap output.
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
    let mut command = localize_command(Cli::command(), localizer.as_ref());
    command = configure_validation_parsers(command, localizer);
    let matches = command
        .try_get_matches_from_mut(iter)
        .map_err(|err| localize_clap_error_with_command(err, localizer.as_ref(), Some(&command)))?;
    let matches_for_merge = matches.clone();
    let mut matches_for_parse = matches;
    let cli = Cli::from_arg_matches_mut(&mut matches_for_parse).map_err(|clap_err| {
        let with_cmd = clap_err.with_cmd(&command);
        localize_clap_error_with_command(with_cmd, localizer.as_ref(), Some(&command))
    })?;
    Ok((cli, matches_for_merge))
}

fn configure_validation_parsers(
    mut command: clap::Command,
    localizer: &Arc<dyn Localizer>,
) -> clap::Command {
    let jobs_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_jobs);
    let locale_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_locale);
    let scheme_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_scheme);
    let host_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_host_pattern);
    let color_policy_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_color_policy);
    let emoji_policy_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_emoji_policy);
    let progress_policy_parser =
        LocalizedValueParser::new(Arc::clone(localizer), parse_progress_policy);
    let accessibility_policy_parser =
        LocalizedValueParser::new(Arc::clone(localizer), parse_accessibility_policy);

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
