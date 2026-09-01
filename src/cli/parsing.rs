//! Clap value-parser helpers, invoked exclusively from [`super::parser`].
//!
//! Most `parse_*` functions implement a localization-aware validator for one
//! typed CLI argument and are registered as
//! [`super::parser::LocalizedValueParser`] instances inside
//! `parse_with_localizer_from`. The `parse_utf8_path` helper instead receives
//! raw [`OsStr`] input and is invoked by `LocalizedUtf8PathParser` in
//! [`super::parser`]. None of these helpers is called directly from outside
//! the `cli` module tree.
//!
//! **Pipeline position:** argument-validation layer, below [`super::parser`].
//!
//! - Receives raw `&str` and [`OsStr`] values from Clap's argument machinery.
//! - Emits localized error strings via [`super::parser::validation_message`].
//! - Shared dispatch logic lives in [`parse_value_enum`] (called by the three
//!   enum-valued parsers via [`ParseEnumSpec`]).

use camino::Utf8PathBuf;
use metrics::{counter, describe_counter};
use ortho_config::{LanguageIdentifier, LocalizationArgs, Localizer};
use std::ffi::OsStr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Once;

use super::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
use crate::cli::PATH_VALIDATION_TOTAL;
use crate::host_pattern::HostPattern;
use crate::localization::keys;

/// Parse and validate a jobs argument.
///
/// The accepted range is 1 to `MAX_JOBS`; any other value yields a localized
/// error.
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
    if (1..=super::validation::MAX_JOBS).contains(&value) {
        Ok(value)
    } else {
        let mut args = LocalizationArgs::default();
        args.insert("min", 1.to_string().into());
        args.insert("max", super::validation::MAX_JOBS.to_string().into());
        Err(super::parser::validation_message(
            localizer,
            keys::CLI_JOBS_OUT_OF_RANGE,
            Some(&args),
            &format!("jobs must be between 1 and {}", super::validation::MAX_JOBS),
        ))
    }
}

/// Parse a path argument, rejecting non-UTF-8 input with a localized error.
///
/// `--file` and `--directory` feed the UTF-8 Ninja invocation chain, so this
/// preserves the original `OsStr` long enough to report the rejection at the
/// CLI boundary rather than after partial runner setup.
pub(super) fn parse_utf8_path(
    localizer: &dyn Localizer,
    value: &OsStr,
    key: &'static str,
    fallback: &str,
) -> Result<Utf8PathBuf, String> {
    Utf8PathBuf::from_path_buf(PathBuf::from(value)).map_err(|path| {
        record_non_utf8_path_validation(key);
        let mut args = LocalizationArgs::default();
        args.insert("path", path.display().to_string().into());
        super::parser::validation_message(localizer, key, Some(&args), fallback)
    })
}

/// Record a bounded metric for one rejected UTF-8-only CLI path argument.
fn record_non_utf8_path_validation(key: &str) {
    let source = match key {
        keys::CLI_FILE_NON_UTF8 => "file",
        keys::CLI_DIRECTORY_NON_UTF8 => "directory",
        _ => return,
    };
    describe_path_validation_metric();
    counter!(PATH_VALIDATION_TOTAL, "source" => source, "reason" => "non_utf8").increment(1);
}

/// Describe the stable CLI path-validation metric once per process.
fn describe_path_validation_metric() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            PATH_VALIDATION_TOTAL,
            "Counts rejected UTF-8-only CLI path values by bounded source and reason."
        );
    });
}

/// Parse and normalize a URI scheme provided via CLI flags.
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

/// Parse and normalize a locale tag, rejecting empty and malformed values.
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

/// Parse a colour policy, yielding a localized error for invalid values.
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

/// Parse an emoji policy, yielding a localized error for invalid values.
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

/// Parse a progress policy, yielding a localized error for invalid values.
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

/// Parse an accessibility policy, yielding a localized error for invalid values.
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

/// Bundles the static localization metadata needed by [`parse_value_enum`].
#[derive(Copy, Clone)]
struct ParseEnumSpec {
    /// Localization key naming the invalid-value message.
    key: &'static str,
    /// Interpolation argument that carries the rejected value.
    arg_name: &'static str,
}

/// Parse a value-enum member, yielding a localized error for invalid input.
fn parse_value_enum<T>(localizer: &dyn Localizer, s: &str, spec: ParseEnumSpec) -> Result<T, String>
where
    T: FromStr,
{
    s.parse::<T>().map_err(|_| {
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
/// normalized host body so downstream configuration can reuse the parsed
/// structure without reparsing strings.
pub(super) fn parse_host_pattern(
    _localizer: &dyn Localizer,
    s: &str,
) -> Result<HostPattern, String> {
    HostPattern::parse(s).map_err(|err| err.to_string())
}

#[cfg(all(test, unix))]
mod tests {
    //! Property coverage for the raw operating-system path parser boundary.

    use super::parse_utf8_path;
    use crate::cli::PATH_VALIDATION_TOTAL;
    use crate::cli_localization::build_localizer;
    use crate::localization::keys;
    use camino::Utf8PathBuf;
    use metrics_util::{
        MetricKind,
        debugging::{DebugValue, DebuggingRecorder},
    };
    use proptest::prelude::*;
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    const PATH_ARGUMENTS: [(&str, &str, &str, &str); 2] = [
        (
            "--file",
            keys::CLI_FILE_NON_UTF8,
            "Manifest path is not valid UTF-8.",
            "Manifest path",
        ),
        (
            "--directory",
            keys::CLI_DIRECTORY_NON_UTF8,
            "Working directory path is not valid UTF-8.",
            "Working directory path",
        ),
    ];

    /// Record exactly one bounded rejection counter for each UTF-8-only path argument.
    #[rstest::rstest]
    #[case::file(keys::CLI_FILE_NON_UTF8, "Manifest path is not valid UTF-8.", "file")]
    #[case::directory(
        keys::CLI_DIRECTORY_NON_UTF8,
        "Working directory path is not valid UTF-8.",
        "directory"
    )]
    fn rejected_utf8_path_records_bounded_metric(
        #[case] key: &'static str,
        #[case] fallback: &str,
        #[case] source: &str,
    ) {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        let localizer = build_localizer(None);

        metrics::with_local_recorder(&recorder, || {
            parse_utf8_path(
                localizer.as_ref(),
                &OsString::from_vec(b"manifest-\xff".to_vec()),
                key,
                fallback,
            )
            .expect_err("non-UTF-8 paths must be rejected");
        });

        let snapshot = snapshotter.snapshot().into_vec();
        assert!(
            snapshot.iter().any(|(metric, _, _, value)| {
                metric.kind() == MetricKind::Counter
                    && metric.key().name() == PATH_VALIDATION_TOTAL
                    && metric.key().labels().count() == 2
                    && metric
                        .key()
                        .labels()
                        .any(|label| label.key() == "source" && label.value() == source)
                    && metric
                        .key()
                        .labels()
                        .any(|label| label.key() == "reason" && label.value() == "non_utf8")
                    && matches!(value, DebugValue::Counter(1))
            }),
            "rejected path should record one bounded validation counter: {snapshot:?}"
        );
    }

    proptest! {
        #[test]
        fn utf8_path_parser_preserves_valid_paths_and_rejects_invalid_bytes(
            bytes in prop::collection::vec(any::<u8>(), 0..512)
        ) {
            let localizer = build_localizer(None);
            let value = OsString::from_vec(bytes.clone());

            for (flag, key, fallback, diagnostic_subject) in PATH_ARGUMENTS {
                match (
                    String::from_utf8(bytes.clone()),
                    parse_utf8_path(localizer.as_ref(), &value, key, fallback),
                ) {
                    (Ok(valid), Ok(path)) => prop_assert_eq!(path, Utf8PathBuf::from(valid)),
                    (Err(_), Err(error)) => prop_assert!(
                        error.contains(diagnostic_subject) && error.contains("not valid UTF-8"),
                        "{flag} should use its localized UTF-8 diagnostic, got: {error}"
                    ),
                    (Ok(valid), Err(error)) => prop_assert!(
                        false,
                        "{flag} should accept valid UTF-8 path {valid:?}, got: {error}"
                    ),
                    (Err(_), Ok(path)) => prop_assert!(
                        false,
                        "{flag} should reject invalid UTF-8 bytes, got: {path}"
                    ),
                }
            }
        }
    }
}
