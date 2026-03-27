//! CLI value parser configuration with localised validation.

use std::sync::Arc;

use clap::builder::{TypedValueParser, ValueParser};
use clap::error::ErrorKind;
use ortho_config::Localizer;

use super::parsing::{
    parse_colour_policy, parse_host_pattern, parse_jobs, parse_locale, parse_output_format,
    parse_scheme, parse_spinner_mode, parse_theme,
};

/// A value parser that delegates to a localised parsing function.
#[derive(Clone)]
pub(super) struct LocalizedValueParser<F> {
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

/// Configure validation parsers for CLI arguments that require localised error messages.
pub(super) fn configure_validation_parsers(
    mut command: clap::Command,
    localizer: &Arc<dyn Localizer>,
) -> clap::Command {
    let jobs_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_jobs);
    let locale_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_locale);
    let scheme_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_scheme);
    let host_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_host_pattern);
    let theme_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_theme);
    let colour_policy_parser =
        LocalizedValueParser::new(Arc::clone(localizer), parse_colour_policy);
    let spinner_mode_parser = LocalizedValueParser::new(Arc::clone(localizer), parse_spinner_mode);
    let output_format_parser =
        LocalizedValueParser::new(Arc::clone(localizer), parse_output_format);

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
