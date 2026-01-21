//! CLI parsing helpers for clap value parsers.

use ortho_config::{LanguageIdentifier, LocalizationArgs};
use std::str::FromStr;

use crate::host_pattern::HostPattern;
use crate::localization::keys;

pub(super) fn parse_jobs(s: &str) -> Result<usize, String> {
    let value: usize = s.parse().map_err(|_| {
        let mut args = LocalizationArgs::default();
        args.insert("value", s.to_owned().into());
        super::validation_message(
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
pub(super) fn parse_scheme(s: &str) -> Result<String, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(super::validation_message(
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
            keys::CLI_SCHEME_INVALID_START,
            Some(&args),
            &format!("scheme '{s}' must start with an ASCII letter"),
        ));
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        let mut args = LocalizationArgs::default();
        args.insert("scheme", s.to_owned().into());
        return Err(super::validation_message(
            keys::CLI_SCHEME_INVALID,
            Some(&args),
            &format!("invalid scheme '{s}'"),
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

pub(super) fn parse_locale(s: &str) -> Result<String, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(super::validation_message(
            keys::CLI_LOCALE_EMPTY,
            None,
            "locale must not be empty",
        ));
    }
    LanguageIdentifier::from_str(trimmed)
        .map(|_| trimmed.to_owned())
        .map_err(|_| {
            let mut args = LocalizationArgs::default();
            args.insert("locale", trimmed.to_owned().into());
            super::validation_message(
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
pub(super) fn parse_host_pattern(s: &str) -> Result<HostPattern, String> {
    HostPattern::parse(s).map_err(|err| err.to_string())
}
