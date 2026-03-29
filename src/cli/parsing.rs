//! CLI parsing helpers for clap value parsers.

use ortho_config::{LanguageIdentifier, LocalizationArgs, Localizer};
use std::str::FromStr;

use crate::cli::config::{ColourPolicy, OutputFormat, SpinnerMode};
use crate::host_pattern::HostPattern;
use crate::localization::keys;
use crate::theme::ThemePreference;

/// A raw, unparsed CLI argument value.
#[derive(Clone, Copy)]
pub(super) struct RawCliValue<'a>(pub &'a str);

/// A localizer-bound parser for CLI values requiring localized validation.
pub(super) struct LocalizedParser<'a> {
    localizer: &'a dyn Localizer,
}

impl<'a> LocalizedParser<'a> {
    /// Create a parser bound to the provided localizer.
    pub(super) fn new(localizer: &'a dyn Localizer) -> Self {
        Self { localizer }
    }

    pub(super) fn parse_jobs(&self, raw: RawCliValue<'_>) -> Result<usize, String> {
        let s = raw.0;
        let value: usize = s.parse().map_err(|_| {
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
    pub(super) fn parse_scheme(&self, raw: RawCliValue<'_>) -> Result<String, String> {
        let s = raw.0;
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

    pub(super) fn parse_locale(&self, raw: RawCliValue<'_>) -> Result<String, String> {
        let s = raw.0;
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

    /// Parse a host pattern supplied via CLI flags.
    ///
    /// The returned [`HostPattern`] retains both the wildcard flag and the
    /// normalised host body so downstream configuration can reuse the parsed
    /// structure without reparsing strings.
    pub(super) fn parse_host_pattern(&self, raw: RawCliValue<'_>) -> Result<HostPattern, String> {
        let _ = self.localizer;
        HostPattern::parse(raw.0).map_err(|err| err.to_string())
    }

    fn parse_enum<T, E>(
        localizer: &dyn Localizer,
        s: &str,
        parse: impl FnOnce(&str) -> Result<T, E>,
        error_spec: ParseEnumErrorSpec<'_>,
    ) -> Result<T, String> {
        parse(s).map_err(|_| {
            let mut args = LocalizationArgs::default();
            args.insert(error_spec.arg_name, s.to_owned().into());
            super::validation_message(
                localizer,
                error_spec.l10n_key,
                Some(&args),
                error_spec.fallback,
            )
        })
    }

    /// Parse a theme preference supplied via CLI flags or config files.
    ///
    /// Accepts `auto`, `unicode`, or `ascii` (case-insensitive).
    pub(super) fn parse_theme(&self, raw: RawCliValue<'_>) -> Result<ThemePreference, String> {
        let s = raw.0;
        Self::parse_enum(
            self.localizer,
            s,
            ThemePreference::parse_raw,
            ParseEnumErrorSpec {
                arg_name: "theme",
                l10n_key: keys::CLI_THEME_INVALID,
                fallback: &format!("invalid theme '{s}'"),
            },
        )
    }

    fn parse_config_enum<T: ParseRaw>(
        &self,
        raw: RawCliValue<'_>,
        l10n_key: &'static str,
        fallback: &str,
    ) -> Result<T, String> {
        let s = raw.0;
        Self::parse_enum(
            self.localizer,
            s,
            T::parse_raw,
            ParseEnumErrorSpec {
                arg_name: "value",
                l10n_key,
                fallback,
            },
        )
    }

    /// Parse a colour policy supplied via CLI flags or config files.
    pub(super) fn parse_colour_policy(&self, raw: RawCliValue<'_>) -> Result<ColourPolicy, String> {
        let s = raw.0;
        self.parse_config_enum(
            raw,
            keys::CLI_COLOUR_POLICY_INVALID,
            &format!("invalid colour policy '{s}'"),
        )
    }

    /// Parse a spinner mode supplied via CLI flags or config files.
    pub(super) fn parse_spinner_mode(&self, raw: RawCliValue<'_>) -> Result<SpinnerMode, String> {
        let s = raw.0;
        self.parse_config_enum(
            raw,
            keys::CLI_SPINNER_MODE_INVALID,
            &format!("invalid spinner mode '{s}'"),
        )
    }

    /// Parse an output format supplied via CLI flags or config files.
    pub(super) fn parse_output_format(&self, raw: RawCliValue<'_>) -> Result<OutputFormat, String> {
        let s = raw.0;
        self.parse_config_enum(
            raw,
            keys::CLI_OUTPUT_FORMAT_INVALID,
            &format!("invalid output format '{s}'"),
        )
    }
}

#[derive(Clone, Copy)]
struct ParseEnumErrorSpec<'a> {
    arg_name: &'static str,
    l10n_key: &'static str,
    fallback: &'a str,
}

trait ParseRaw: Sized {
    fn parse_raw(s: &str) -> Result<Self, &'static [&'static str]>;
}

impl ParseRaw for ColourPolicy {
    fn parse_raw(s: &str) -> Result<Self, &'static [&'static str]> {
        Self::parse_raw(s)
    }
}

impl ParseRaw for SpinnerMode {
    fn parse_raw(s: &str) -> Result<Self, &'static [&'static str]> {
        Self::parse_raw(s)
    }
}

impl ParseRaw for OutputFormat {
    fn parse_raw(s: &str) -> Result<Self, &'static [&'static str]> {
        Self::parse_raw(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::config::{ColourPolicy, OutputFormat, SpinnerMode};
    use rstest::rstest;

    /// Mock localizer for testing localized parser error messages.
    struct MockLocalizer;

    impl Localizer for MockLocalizer {
        fn lookup(&self, _key: &str, _args: Option<&LocalizationArgs>) -> Option<String> {
            Some(String::from("mock localized message"))
        }
    }

    #[rstest]
    #[case::auto("auto", ThemePreference::Auto)]
    #[case::unicode("unicode", ThemePreference::Unicode)]
    #[case::ascii("ascii", ThemePreference::Ascii)]
    #[case::auto_uppercase("AUTO", ThemePreference::Auto)]
    #[case::unicode_mixed("Unicode", ThemePreference::Unicode)]
    #[case::ascii_with_whitespace("  ascii  ", ThemePreference::Ascii)]
    fn parse_theme_valid_inputs(#[case] input: &str, #[case] expected: ThemePreference) {
        let localizer = MockLocalizer;
        let parser = LocalizedParser::new(&localizer);
        let result = parser.parse_theme(RawCliValue(input));
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
    fn parse_theme_invalid_inputs(#[case] input: &str) {
        let localizer = MockLocalizer;
        let parser = LocalizedParser::new(&localizer);
        let result = parser.parse_theme(RawCliValue(input));
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
    fn parse_colour_policy_valid_inputs(#[case] input: &str, #[case] expected: ColourPolicy) {
        let localizer = MockLocalizer;
        let parser = LocalizedParser::new(&localizer);
        let result = parser.parse_colour_policy(RawCliValue(input));
        match result {
            Ok(policy) => assert_eq!(policy, expected),
            Err(e) => panic!("Expected Ok({expected:?}), got Err: {e}"),
        }
    }

    #[rstest]
    #[case::invalid("loud")]
    #[case::empty("")]
    fn parse_colour_policy_invalid_inputs(#[case] input: &str) {
        let localizer = MockLocalizer;
        let parser = LocalizedParser::new(&localizer);
        assert!(parser.parse_colour_policy(RawCliValue(input)).is_err());
    }

    #[rstest]
    #[case::enabled("enabled", SpinnerMode::Enabled)]
    #[case::disabled("DISABLED", SpinnerMode::Disabled)]
    fn parse_spinner_mode_valid_inputs(#[case] input: &str, #[case] expected: SpinnerMode) {
        let localizer = MockLocalizer;
        let parser = LocalizedParser::new(&localizer);
        let result = parser.parse_spinner_mode(RawCliValue(input));
        match result {
            Ok(mode) => assert_eq!(mode, expected),
            Err(e) => panic!("Expected Ok({expected:?}), got Err: {e}"),
        }
    }

    #[rstest]
    #[case::invalid("paused")]
    #[case::empty("")]
    fn parse_spinner_mode_invalid_inputs(#[case] input: &str) {
        let localizer = MockLocalizer;
        let parser = LocalizedParser::new(&localizer);
        assert!(parser.parse_spinner_mode(RawCliValue(input)).is_err());
    }

    #[rstest]
    #[case::human("human", OutputFormat::Human)]
    #[case::json("JSON", OutputFormat::Json)]
    fn parse_output_format_valid_inputs(#[case] input: &str, #[case] expected: OutputFormat) {
        let localizer = MockLocalizer;
        let parser = LocalizedParser::new(&localizer);
        let result = parser.parse_output_format(RawCliValue(input));
        match result {
            Ok(format) => assert_eq!(format, expected),
            Err(e) => panic!("Expected Ok({expected:?}), got Err: {e}"),
        }
    }

    #[rstest]
    #[case::invalid("tap")]
    #[case::empty("")]
    fn parse_output_format_invalid_inputs(#[case] input: &str) {
        let localizer = MockLocalizer;
        let parser = LocalizedParser::new(&localizer);
        assert!(parser.parse_output_format(RawCliValue(input)).is_err());
    }
}
