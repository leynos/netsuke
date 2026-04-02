//! CLI value parser configuration with localised validation.

use std::sync::Arc;

use clap::builder::{TypedValueParser, ValueParser};
use clap::error::ErrorKind;
use ortho_config::Localizer;

use crate::cli::config::{ColourPolicy, OutputFormat, SpinnerMode};
use crate::theme::ThemePreference;

use super::parsing::{LocalizedParser, parse_host_pattern};

type ParserFn<T> =
    dyn for<'a, 'b> Fn(&LocalizedParser<'a>, &'b str) -> Result<T, String> + Send + Sync;

/// A value parser that delegates to a localised parsing function.
#[derive(Clone)]
pub(super) struct LocalizedValueParser<T> {
    localizer: Arc<dyn Localizer>,
    parser: Arc<ParserFn<T>>,
}

impl<T> LocalizedValueParser<T> {
    fn new<F>(localizer: Arc<dyn Localizer>, parser: F) -> Self
    where
        F: for<'a, 'b> Fn(&LocalizedParser<'a>, &'b str) -> Result<T, String>
            + Send
            + Sync
            + 'static,
    {
        Self {
            localizer,
            parser: Arc::new(parser),
        }
    }
}

fn make_localized_parser<F, T>(localizer: &Arc<dyn Localizer>, parser: F) -> LocalizedValueParser<T>
where
    F: for<'a, 'b> Fn(&LocalizedParser<'a>, &'b str) -> Result<T, String> + Send + Sync + 'static,
{
    LocalizedValueParser::new(Arc::clone(localizer), parser)
}

fn host_pattern_parser(
    _: &LocalizedParser<'_>,
    raw: &str,
) -> Result<crate::host_pattern::HostPattern, String> {
    parse_host_pattern(raw)
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
        (self.parser)(&parser, raw_value)
            .map_err(|err| command.error(ErrorKind::ValueValidation, err))
    }
}

/// Configure validation parsers for CLI arguments that require localised error messages.
pub(super) fn configure_validation_parsers(
    mut command: clap::Command,
    localizer: &Arc<dyn Localizer>,
) -> clap::Command {
    let jobs_parser = make_localized_parser(localizer, |parser: &LocalizedParser<'_>, raw| {
        parser.parse_jobs(raw)
    });
    let locale_parser = make_localized_parser(localizer, |parser: &LocalizedParser<'_>, raw| {
        parser.parse_locale(raw)
    });
    let scheme_parser = make_localized_parser(localizer, |parser: &LocalizedParser<'_>, raw| {
        parser.parse_scheme(raw)
    });
    let host_parser = make_localized_parser(localizer, host_pattern_parser);
    let theme_parser = make_localized_parser(localizer, |parser: &LocalizedParser<'_>, raw| {
        parser.parse_cli_config_enum::<ThemePreference>(raw)
    });
    let colour_policy_parser =
        make_localized_parser(localizer, |parser: &LocalizedParser<'_>, raw| {
            parser.parse_cli_config_enum::<ColourPolicy>(raw)
        });
    let spinner_mode_parser =
        make_localized_parser(localizer, |parser: &LocalizedParser<'_>, raw| {
            parser.parse_cli_config_enum::<SpinnerMode>(raw)
        });
    let output_format_parser =
        make_localized_parser(localizer, |parser: &LocalizedParser<'_>, raw| {
            parser.parse_cli_config_enum::<OutputFormat>(raw)
        });

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
