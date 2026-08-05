//! Environment access for the manifest `env()` Jinja helper.
//!
//! The reader port keeps process access at the composition root while tests
//! inject deterministic values without mutating global state.

use std::{env::VarError, sync::Arc};

use minijinja::{Error, ErrorKind};
use mockable::{DefaultEnv, Env};

use crate::localization::{self, keys};

/// Manifest-owned failure returned by an [`EnvReader`].
///
/// This type distinguishes a missing variable from a value that cannot be
/// represented as UTF-8 without exposing the process adapter's error type.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum EnvReadError {
    /// The requested variable is absent.
    #[error("environment variable is not present")]
    NotPresent,
    /// The requested variable cannot be represented as UTF-8.
    #[error("environment variable contains invalid UTF-8")]
    NotUnicode,
}

impl From<VarError> for EnvReadError {
    fn from(error: VarError) -> Self {
        match error {
            VarError::NotPresent => Self::NotPresent,
            VarError::NotUnicode(_) => Self::NotUnicode,
        }
    }
}

/// Thread-safe environment reader supplied to the `env()` Jinja helper.
///
/// Readers return owned UTF-8 values and report lookup failures through
/// [`EnvReadError`]. Tests can inject a closure-backed reader without mutating
/// the process environment.
///
/// # Examples
///
/// ```rust
/// use netsuke::manifest::{EnvReadError, EnvReader};
/// use std::sync::Arc;
///
/// let reader: EnvReader = Arc::new(|name| match name {
///     "PROFILE" => Ok("release".to_owned()),
///     _ => Err(EnvReadError::NotPresent),
/// });
///
/// assert_eq!(reader("PROFILE"), Ok("release".to_owned()));
/// assert_eq!(reader("MISSING"), Err(EnvReadError::NotPresent));
/// ```
pub type EnvReader = Arc<dyn Fn(&str) -> Result<String, EnvReadError> + Send + Sync>;

/// Construct the process-backed environment reader used by production loads.
///
/// # Examples
///
/// ```rust,no_run
/// use netsuke::manifest::{EnvReadError, process_env_reader};
///
/// let reader = process_env_reader();
/// match reader("PATH") {
///     Ok(path) => assert!(!path.is_empty(), "PATH should not be empty"),
///     Err(EnvReadError::NotPresent | EnvReadError::NotUnicode) => {}
/// }
/// ```
#[must_use]
pub fn process_env_reader() -> EnvReader {
    let env = DefaultEnv;
    Arc::new(move |key| env.raw(key).map_err(EnvReadError::from))
}

/// Resolve `name` through `read_env`, mapping failures to Jinja errors.
///
/// Failures are traced with only a bounded `failure_kind`, and the localized
/// diagnostics carry fixed text. The variable name is deliberately absent from
/// both: it is manifest-controlled and unbounded, and environment variable
/// names routinely identify credentials. The Jinja error's template location
/// tells the author which `env()` call failed.
pub(super) fn env_var_with(
    name: &str,
    read_env: impl FnOnce(&str) -> Result<String, EnvReadError>,
) -> Result<String, Error> {
    match read_env(name) {
        Ok(value) => Ok(value),
        Err(EnvReadError::NotPresent) => {
            tracing::debug!(failure_kind = "not_present", "manifest env lookup failed");
            Err(Error::new(
                ErrorKind::UndefinedError,
                localization::message(keys::MANIFEST_ENV_MISSING).to_string(),
            ))
        }
        Err(EnvReadError::NotUnicode) => {
            tracing::debug!(failure_kind = "not_unicode", "manifest env lookup failed");
            Err(Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::MANIFEST_ENV_INVALID_UTF8).to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    //! Direct tests for the process-backed environment adapter.

    use super::*;
    use crate::test_tracing_capture::with_test_subscriber;
    use rstest::rstest;
    use tracing_subscriber::filter::LevelFilter;

    /// Stands in for a credential named by a manifest; neither the variable
    /// name nor its value may reach a log line.
    const SENTINEL: &str = "s3cr3t-sentinel";

    #[rstest]
    #[case::not_present(EnvReadError::NotPresent, "not_present")]
    #[case::not_unicode(EnvReadError::NotUnicode, "not_unicode")]
    fn lookup_failures_trace_only_a_bounded_failure_kind(
        #[case] failure: EnvReadError,
        #[case] failure_kind: &str,
    ) {
        let events = with_test_subscriber(LevelFilter::DEBUG, |captured| {
            env_var_with(SENTINEL, |_| Err(failure)).expect_err("the injected reader must fail");
            captured.snapshot()
        });

        assert!(
            events
                .iter()
                .any(|event| event.contains("manifest env lookup failed")
                    && event.contains(&format!("failure_kind=\"{failure_kind}\""))),
            "expected a bounded lookup-failure event in {events:?}"
        );
        assert!(
            !events.iter().any(|event| event.contains(SENTINEL)),
            "the variable name must not be logged: {events:?}"
        );
    }

    /// The returned Jinja error must carry only the fixed localized text: a
    /// credential-like variable name supplied by the manifest stays out of it.
    #[rstest]
    #[case::not_present(EnvReadError::NotPresent, ErrorKind::UndefinedError)]
    #[case::not_unicode(EnvReadError::NotUnicode, ErrorKind::InvalidOperation)]
    fn lookup_failures_omit_the_variable_name_from_the_error(
        #[case] failure: EnvReadError,
        #[case] expected_kind: ErrorKind,
    ) {
        let error =
            env_var_with(SENTINEL, |_| Err(failure)).expect_err("the injected reader must fail");

        assert_eq!(
            error.kind(),
            expected_kind,
            "the Jinja error kind must be preserved"
        );
        assert!(
            !error.to_string().contains(SENTINEL),
            "the variable name must not reach the error: {error}"
        );
    }

    #[test]
    fn process_reader_matches_default_environment_adapter() {
        let expected = DefaultEnv.raw("PATH").map_err(EnvReadError::from);
        let actual = process_env_reader()("PATH");

        assert_eq!(
            actual, expected,
            "process reader should delegate to DefaultEnv"
        );
    }
}
