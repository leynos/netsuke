//! Clap value-parser helpers, invoked exclusively from [`super::parser`].
//!
//! Each `parse_*` function implements a localisation-aware validator for one
//! typed CLI argument.  They are registered as [`super::parser::LocalizedValueParser`]
//! instances inside `parse_with_localizer_from` and are never called directly
//! from outside the `cli` module tree.
//!
//! **Pipeline position:** argument-validation layer, below [`super::parser`].
//!
//! - Receives raw `&str` slices from Clap's argument machinery.
//! - Emits localised error strings via [`super::parser::validation_message`].
//! - Shared dispatch logic lives in [`parse_value_enum`] (called by the three
//!   enum-valued parsers via [`ParseEnumSpec`]).

use clap::ValueEnum;
use ortho_config::{LanguageIdentifier, LocalizationArgs, Localizer};
use std::str::FromStr;

use super::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
use crate::host_pattern::HostPattern;
use crate::localization::keys;

pub(super) fn parse_jobs(localizer: &dyn Localizer, s: &str) -> Result<usize, String> {
    let value: usize = s.parse().map_err(|_| {
        let mut args = LocalizationArgs::default();
        args.insert("value", s.to_owned().into());
        super::parser::validation_message(
            localizer,
            keys::CLI_JOBS_INVALID_NUMBER,
            Some(&args),
            &format!("{s} is not a valid number"),
        )
    })?;
    if (1..=super::parser::MAX_JOBS).contains(&value) {
        Ok(value)
    } else {
        let mut args = LocalizationArgs::default();
        args.insert("min", 1.to_string().into());
        args.insert("max", super::parser::MAX_JOBS.to_string().into());
        Err(super::parser::validation_message(
            localizer,
            keys::CLI_JOBS_OUT_OF_RANGE,
            Some(&args),
            &format!("jobs must be between 1 and {}", super::parser::MAX_JOBS),
        ))
    }
}

/// Parse and normalise a URI scheme provided via CLI flags.
///
/// Schemes must begin with an ASCII letter and may contain ASCII letters,
/// digits, `+`, `-`, or `.` characters. The result is returned in lowercase.
pub(super) fn parse_scheme(localizer: &dyn Localizer, s: &str) -> Result<String, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(super::parser::validation_message(
            localizer,
            keys::CLI_SCHEME_EMPTY,
            None,
            "scheme must not be empty",
        ));
    }
    let mut chars = trimmed.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
        let mut args = LocalizationArgs::default();
        args.insert("scheme", s.to_owned().into());
        return Err(super::parser::validation_message(
            localizer,
            keys::CLI_SCHEME_INVALID_START,
            Some(&args),
            &format!("scheme '{s}' must start with an ASCII letter"),
        ));
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        let mut args = LocalizationArgs::default();
        args.insert("scheme", s.to_owned().into());
        return Err(super::parser::validation_message(
            localizer,
            keys::CLI_SCHEME_INVALID,
            Some(&args),
            &format!("invalid scheme '{s}'"),
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

pub(super) fn parse_locale(localizer: &dyn Localizer, s: &str) -> Result<String, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(super::parser::validation_message(
            localizer,
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
            super::parser::validation_message(
                localizer,
                keys::CLI_LOCALE_INVALID,
                Some(&args),
                &format!("invalid locale '{trimmed}'"),
            )
        })
}

pub(super) fn parse_color_policy(
    localizer: &dyn Localizer,
    s: &str,
) -> Result<ColourPolicy, String> {
    parse_value_enum(
        localizer,
        s,
        ParseEnumSpec {
            key: keys::CLI_COLOR_POLICY_INVALID,
            arg_name: "value",
        },
    )
}

pub(super) fn parse_emoji_policy(
    localizer: &dyn Localizer,
    s: &str,
) -> Result<EmojiPolicy, String> {
    parse_value_enum(
        localizer,
        s,
        ParseEnumSpec {
            key: keys::CLI_EMOJI_POLICY_INVALID,
            arg_name: "value",
        },
    )
}

pub(super) fn parse_progress_policy(
    localizer: &dyn Localizer,
    s: &str,
) -> Result<ProgressPolicy, String> {
    parse_value_enum(
        localizer,
        s,
        ParseEnumSpec {
            key: keys::CLI_PROGRESS_POLICY_INVALID,
            arg_name: "value",
        },
    )
}

pub(super) fn parse_accessibility_policy(
    localizer: &dyn Localizer,
    s: &str,
) -> Result<AccessibilityPolicy, String> {
    parse_value_enum(
        localizer,
        s,
        ParseEnumSpec {
            key: keys::CLI_ACCESSIBILITY_POLICY_INVALID,
            arg_name: "value",
        },
    )
}

/// Bundles the static localisation metadata needed by [`parse_value_enum`].
#[derive(Copy, Clone)]
struct ParseEnumSpec {
    key: &'static str,
    arg_name: &'static str,
}

fn parse_value_enum<T>(localizer: &dyn Localizer, s: &str, spec: ParseEnumSpec) -> Result<T, String>
where
    T: ValueEnum,
{
    T::from_str(s, true).map_err(|_| {
        let mut args = LocalizationArgs::default();
        args.insert(spec.arg_name, s.to_owned().into());
        super::parser::validation_message(
            localizer,
            spec.key,
            Some(&args),
            &format!("Invalid '{s}'"),
        )
    })
}

/// Parse a host pattern supplied via CLI flags.
///
/// The returned [`HostPattern`] retains both the wildcard flag and the
/// normalised host body so downstream configuration can reuse the parsed
/// structure without reparsing strings.
pub(super) fn parse_host_pattern(
    _localizer: &dyn Localizer,
    s: &str,
) -> Result<HostPattern, String> {
    HostPattern::parse(s).map_err(|err| err.to_string())
}
