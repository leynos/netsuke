//! CLI value parser configuration with localised validation.

use std::sync::Arc;

use clap::builder::{TypedValueParser, ValueParser};
use clap::error::ErrorKind;
use ortho_config::Localizer;

use super::parsing::{LocalizedParser, RawCliValue};
use crate::cli::config::{ColourPolicy, OutputFormat, SpinnerMode};
use crate::host_pattern::HostPattern;
use crate::theme::ThemePreference;

/// A value parser that delegates to a localised parsing function.
#[derive(Clone)]
pub(super) struct LocalizedValueParser<T> {
    localizer: Arc<dyn Localizer>,
    parser: fn(&LocalizedParser<'_>, RawCliValue<'_>) -> Result<T, String>,
}

impl<T> LocalizedValueParser<T> {
    fn new(
        localizer: Arc<dyn Localizer>,
        parser: fn(&LocalizedParser<'_>, RawCliValue<'_>) -> Result<T, String>,
    ) -> Self {
        Self { localizer, parser }
    }
}

fn parse_jobs_value(parser: &LocalizedParser<'_>, raw: RawCliValue<'_>) -> Result<usize, String> {
    parser.parse_jobs(raw)
}

fn parse_locale_value(
    parser: &LocalizedParser<'_>,
    raw: RawCliValue<'_>,
) -> Result<String, String> {
    parser.parse_locale(raw)
}

fn parse_scheme_value(
    parser: &LocalizedParser<'_>,
    raw: RawCliValue<'_>,
) -> Result<String, String> {
    parser.parse_scheme(raw)
}

fn parse_host_pattern_value(
    parser: &LocalizedParser<'_>,
    raw: RawCliValue<'_>,
) -> Result<HostPattern, String> {
    parser.parse_host_pattern(raw)
}

fn parse_theme_value(
    parser: &LocalizedParser<'_>,
    raw: RawCliValue<'_>,
) -> Result<ThemePreference, String> {
    parser.parse_theme(raw)
}

fn parse_colour_policy_value(
    parser: &LocalizedParser<'_>,
    raw: RawCliValue<'_>,
) -> Result<ColourPolicy, String> {
    parser.parse_colour_policy(raw)
}

fn parse_spinner_mode_value(
    parser: &LocalizedParser<'_>,
    raw: RawCliValue<'_>,
) -> Result<SpinnerMode, String> {
    parser.parse_spinner_mode(raw)
}

fn parse_output_format_value(
    parser: &LocalizedParser<'_>,
    raw: RawCliValue<'_>,
) -> Result<OutputFormat, String> {
    parser.parse_output_format(raw)
}

impl<T> TypedValueParser for LocalizedValueParser<T>
where
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
        let parser = LocalizedParser::new(self.localizer.as_ref());
        (self.parser)(&parser, RawCliValue(raw_value))
            .map_err(|err| command.error(ErrorKind::ValueValidation, err))
    }
}

/// Configure validation parsers for CLI arguments that require localised error messages.
pub(super) fn configure_validation_parsers(
    mut command: clap::Command,
    localizer: &Arc<dyn Localizer>,
) -> clap::Command {
    let jobs_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_jobs_value);
    let locale_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_locale_value);
    let scheme_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_scheme_value);
    let host_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_host_pattern_value);
    let theme_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_theme_value);
    let colour_policy_parser =
        LocalizedValueParser::new(Arc::clone(localizer), parse_colour_policy_value);
    let spinner_mode_parser =
        LocalizedValueParser::new(Arc::clone(localizer), parse_spinner_mode_value);
    let output_format_parser =
        LocalizedValueParser::new(Arc::clone(localizer), parse_output_format_value);

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
    command = command.mut_arg("theme", |arg| {
        arg.value_parser(ValueParser::new(theme_parser))
    });
    command = command.mut_arg("colour_policy", |arg| {
        arg.value_parser(ValueParser::new(colour_policy_parser))
    });
    command = command.mut_arg("spinner_mode", |arg| {
        arg.value_parser(ValueParser::new(spinner_mode_parser))
    });
    command = command.mut_arg("output_format", |arg| {
        arg.value_parser(ValueParser::new(output_format_parser))
    });
    command
}
