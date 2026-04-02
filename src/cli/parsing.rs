//! CLI parsing helpers for clap value parsers.

use ortho_config::{LanguageIdentifier, LocalizationArgs, Localizer};
use std::str::FromStr;

use crate::cli::config::{ColourPolicy, OutputFormat, SpinnerMode};
use crate::host_pattern::HostPattern;
use crate::localization::keys;
use crate::theme::ThemePreference;

/// Trait implemented by config-backed CLI enums that share localized parsing.
///
/// Implementors provide the Fluent key used for localized validation errors,
/// a short human-readable label used in fallback messages, and a parser
/// function pointer that accepts raw user input and returns either the parsed
/// enum or the valid option list for error reporting.
pub(super) trait CliConfigEnum: Sized {
    /// Fluent key used when localized validation fails.
    const L10N_KEY: &'static str;
    /// Human-readable label used in the fallback error message.
    const LABEL: &'static str;
    /// Fluent argument name used for the invalid raw value.
    const ARG_NAME: &'static str = "value";
    /// Parser for raw user input.
    ///
    /// Implementors should trim and normalize input the same way their CLI and
    /// config-file parsing paths do, returning `Err(valid_options)` when the
    /// value cannot be parsed.
    const PARSE_RAW: fn(&str) -> Result<Self, &'static [&'static str]>;
}

impl CliConfigEnum for ColourPolicy {
    const L10N_KEY: &'static str = keys::CLI_COLOUR_POLICY_INVALID;
    const LABEL: &'static str = "colour policy";
    const PARSE_RAW: fn(&str) -> Result<Self, &'static [&'static str]> = Self::parse_raw;
}

impl CliConfigEnum for SpinnerMode {
    const L10N_KEY: &'static str = keys::CLI_SPINNER_MODE_INVALID;
    const LABEL: &'static str = "spinner mode";
    const PARSE_RAW: fn(&str) -> Result<Self, &'static [&'static str]> = Self::parse_raw;
}

impl CliConfigEnum for OutputFormat {
    const L10N_KEY: &'static str = keys::CLI_OUTPUT_FORMAT_INVALID;
    const LABEL: &'static str = "output format";
    const PARSE_RAW: fn(&str) -> Result<Self, &'static [&'static str]> = Self::parse_raw;
}

impl CliConfigEnum for ThemePreference {
    const L10N_KEY: &'static str = keys::CLI_THEME_INVALID;
    const LABEL: &'static str = "theme";
    const ARG_NAME: &'static str = "theme";
    const PARSE_RAW: fn(&str) -> Result<Self, &'static [&'static str]> = Self::parse_raw;
}

/// A localizer-bound parser for CLI values requiring localized validation.
pub(super) struct LocalizedParser<'a> {
    localizer: &'a dyn Localizer,
}

impl<'a> LocalizedParser<'a> {
    /// Create a parser bound to the provided localizer.
    pub(super) fn new(localizer: &'a dyn Localizer) -> Self {
        Self { localizer }
    }

    /// Parse the `--jobs` CLI value into a bounded worker-count.
    ///
    /// Leading and trailing whitespace is ignored. Returns a localized `String`
    /// error when the value is not a valid integer or falls outside
    /// `1..=MAX_JOBS`.
    pub(super) fn parse_jobs(&self, s: &str) -> Result<usize, String> {
        let trimmed = s.trim();
        let value: usize = trimmed.parse().map_err(|_| {
            let mut args = LocalizationArgs::default();
            args.insert("value", s.to_owned().into());
            super::validation_message(
                self.localizer,
                keys::CLI_JOBS_INVALID_NUMBER,
                Some(&args),
                &format!("{s} is not a valid number"),
            )
        })?;
        if (1..=super::MAX_JOBS).contains(&value) {
            Ok(value)
        } else {
            let mut args = LocalizationArgs::default();
            args.insert("min", 1.to_string().into());
            args.insert("max", super::MAX_JOBS.to_string().into());
            Err(super::validation_message(
                self.localizer,
                keys::CLI_JOBS_OUT_OF_RANGE,
                Some(&args),
                &format!("jobs must be between 1 and {}", super::MAX_JOBS),
            ))
        }
    }

    /// Parse and normalise a URI scheme provided via CLI flags.
    ///
    /// Schemes must begin with an ASCII letter and may contain ASCII letters,
    /// digits, `+`, `-`, or `.` characters. The result is returned in lowercase.
    pub(super) fn parse_scheme(&self, s: &str) -> Result<String, String> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(super::validation_message(
                self.localizer,
                keys::CLI_SCHEME_EMPTY,
                None,
                "scheme must not be empty",
            ));
        }
        let mut chars = trimmed.chars();
        if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
            let mut args = LocalizationArgs::default();
            args.insert("scheme", s.to_owned().into());
            return Err(super::validation_message(
                self.localizer,
                keys::CLI_SCHEME_INVALID_START,
                Some(&args),
                &format!("scheme '{s}' must start with an ASCII letter"),
            ));
        }
        if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
            let mut args = LocalizationArgs::default();
            args.insert("scheme", s.to_owned().into());
            return Err(super::validation_message(
                self.localizer,
                keys::CLI_SCHEME_INVALID,
                Some(&args),
                &format!("invalid scheme '{s}'"),
            ));
        }
        Ok(trimmed.to_ascii_lowercase())
    }

    /// Parse a locale identifier supplied via CLI flags.
    ///
    /// Surrounding whitespace is ignored. On success this returns the
    /// canonicalized locale string emitted by `LanguageIdentifier`; on failure
    /// it returns a localized `String` describing the invalid input.
    pub(super) fn parse_locale(&self, s: &str) -> Result<String, String> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(super::validation_message(
                self.localizer,
                keys::CLI_LOCALE_EMPTY,
                None,
                "locale must not be empty",
            ));
        }
        LanguageIdentifier::from_str(trimmed)
            .map(|lang| lang.to_string())
            .map_err(|_| {
                let mut args = LocalizationArgs::default();
                args.insert("locale", trimmed.to_owned().into());
                super::validation_message(
                    self.localizer,
                    keys::CLI_LOCALE_INVALID,
                    Some(&args),
                    &format!("invalid locale '{trimmed}'"),
                )
            })
    }

    /// Parse a config-backed CLI enum using its shared localization contract.
    ///
    /// The parser delegates to [`CliConfigEnum::PARSE_RAW`]. Successful parses
    /// return the concrete enum value. Failures return a localized `String`
    /// built from [`CliConfigEnum::L10N_KEY`] and the raw user input.
    pub(super) fn parse_cli_config_enum<T: CliConfigEnum>(&self, s: &str) -> Result<T, String> {
        (T::PARSE_RAW)(s).map_err(|_| {
            let mut args = LocalizationArgs::default();
            args.insert(T::ARG_NAME, s.to_owned().into());
            super::validation_message(
                self.localizer,
                T::L10N_KEY,
                Some(&args),
                &format!("invalid {} '{s}'", T::LABEL),
            )
        })
    }
}

/// Parse a host pattern supplied via CLI flags.
///
/// The returned [`HostPattern`] retains both the wildcard flag and the
/// normalised host body so downstream configuration can reuse the parsed
/// structure without reparsing strings.
pub(super) fn parse_host_pattern(s: &str) -> Result<HostPattern, String> {
    HostPattern::parse(s).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::config::{ColourPolicy, OutputFormat, SpinnerMode};
    use rstest::{fixture, rstest};
    use std::fmt::Write as _;

    /// Mock localizer for testing localized parser error messages.
    struct MockLocalizer;

    impl Localizer for MockLocalizer {
        fn lookup(&self, key: &str, lookup_args: Option<&LocalizationArgs>) -> Option<String> {
            let mut rendered = String::from(key);
            if let Some(args) = lookup_args {
                rendered.push_str(": ");
                write!(&mut rendered, "{args:?}")
                    .expect("writing debug args into a String should succeed");
            }
            Some(rendered)
        }
    }

    #[fixture]
    fn parser() -> LocalizedParser<'static> {
        static LOCALIZER: MockLocalizer = MockLocalizer;
        LocalizedParser::new(&LOCALIZER)
    }

    #[rstest]
    #[case::trimmed(" 4 ", 4)]
    fn parse_jobs_valid_inputs(
        parser: LocalizedParser<'static>,
        #[case] input: &str,
        #[case] expected: usize,
    ) {
        let result = parser.parse_jobs(input);
        match result {
            Ok(jobs) => assert_eq!(jobs, expected),
            Err(e) => panic!("Expected Ok({expected}), got Err: {e}"),
        }
    }

    #[rstest]
    #[case::auto("auto", ThemePreference::Auto)]
    #[case::unicode("unicode", ThemePreference::Unicode)]
    #[case::ascii("ascii", ThemePreference::Ascii)]
    #[case::auto_uppercase("AUTO", ThemePreference::Auto)]
    #[case::unicode_mixed("Unicode", ThemePreference::Unicode)]
    #[case::ascii_with_whitespace("  ascii  ", ThemePreference::Ascii)]
    fn parse_theme_valid_inputs(
        parser: LocalizedParser<'static>,
        #[case] input: &str,
        #[case] expected: ThemePreference,
    ) {
        let result = parser.parse_cli_config_enum::<ThemePreference>(input);
        match result {
            Ok(theme) => assert_eq!(theme, expected),
            Err(e) => panic!("Expected Ok({expected:?}), got Err: {e}"),
        }
    }

    #[rstest]
    #[case::invalid_word("invalid")]
    #[case::empty("")]
    #[case::number("123")]
    #[case::close_typo("unicod")]
    fn parse_theme_invalid_inputs(parser: LocalizedParser<'static>, #[case] input: &str) {
        let result = parser.parse_cli_config_enum::<ThemePreference>(input);
        match result {
            Err(error_msg) => {
                assert!(!error_msg.is_empty(), "Error message should not be empty");
            }
            Ok(theme) => panic!("Expected Err for input '{input}', got Ok({theme:?})"),
        }
    }

    #[rstest]
    #[case::auto("auto", ColourPolicy::Auto)]
    #[case::always("ALWAYS", ColourPolicy::Always)]
    #[case::never(" never ", ColourPolicy::Never)]
    fn parse_colour_policy_valid_inputs(
        parser: LocalizedParser<'static>,
        #[case] input: &str,
        #[case] expected: ColourPolicy,
    ) {
        let result = parser.parse_cli_config_enum::<ColourPolicy>(input);
        match result {
            Ok(policy) => assert_eq!(policy, expected),
            Err(e) => panic!("Expected Ok({expected:?}), got Err: {e}"),
        }
    }

    #[rstest]
    #[case::invalid("loud")]
    #[case::empty("")]
    fn parse_colour_policy_invalid_inputs(parser: LocalizedParser<'static>, #[case] input: &str) {
        let result = parser.parse_cli_config_enum::<ColourPolicy>(input);
        match result {
            Err(error_msg) => assert!(
                error_msg.starts_with(keys::CLI_COLOUR_POLICY_INVALID),
                "expected error to start with {:?}, got {error_msg:?}",
                keys::CLI_COLOUR_POLICY_INVALID,
            ),
            Ok(policy) => panic!("Expected Err for input '{input}', got Ok({policy:?})"),
        }
    }

    #[rstest]
    #[case::enabled("enabled", SpinnerMode::Enabled)]
    #[case::disabled("DISABLED", SpinnerMode::Disabled)]
    fn parse_spinner_mode_valid_inputs(
        parser: LocalizedParser<'static>,
        #[case] input: &str,
        #[case] expected: SpinnerMode,
    ) {
        let result = parser.parse_cli_config_enum::<SpinnerMode>(input);
        match result {
            Ok(mode) => assert_eq!(mode, expected),
            Err(e) => panic!("Expected Ok({expected:?}), got Err: {e}"),
        }
    }

    #[rstest]
    #[case::invalid("paused")]
    #[case::empty("")]
    fn parse_spinner_mode_invalid_inputs(parser: LocalizedParser<'static>, #[case] input: &str) {
        let result = parser.parse_cli_config_enum::<SpinnerMode>(input);
        match result {
            Err(error_msg) => assert!(
                error_msg.starts_with(keys::CLI_SPINNER_MODE_INVALID),
                "expected error to start with {:?}, got {error_msg:?}",
                keys::CLI_SPINNER_MODE_INVALID,
            ),
            Ok(mode) => panic!("Expected Err for input '{input}', got Ok({mode:?})"),
        }
    }

    #[rstest]
    #[case::human("human", OutputFormat::Human)]
    #[case::json("JSON", OutputFormat::Json)]
    fn parse_output_format_valid_inputs(
        parser: LocalizedParser<'static>,
        #[case] input: &str,
        #[case] expected: OutputFormat,
    ) {
        let result = parser.parse_cli_config_enum::<OutputFormat>(input);
        match result {
            Ok(format) => assert_eq!(format, expected),
            Err(e) => panic!("Expected Ok({expected:?}), got Err: {e}"),
        }
    }

    #[rstest]
    #[case::invalid("tap")]
    #[case::empty("")]
    fn parse_output_format_invalid_inputs(parser: LocalizedParser<'static>, #[case] input: &str) {
        let result = parser.parse_cli_config_enum::<OutputFormat>(input);
        match result {
            Err(error_msg) => assert!(
                error_msg.starts_with(keys::CLI_OUTPUT_FORMAT_INVALID),
                "expected error to start with {:?}, got {error_msg:?}",
                keys::CLI_OUTPUT_FORMAT_INVALID,
            ),
            Ok(format) => panic!("Expected Err for input '{input}', got Ok({format:?})"),
        }
    }
}
