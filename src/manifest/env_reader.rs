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
pub(super) fn env_var_with(
    name: &str,
    read_env: impl FnOnce(&str) -> Result<String, EnvReadError>,
) -> Result<String, Error> {
    match read_env(name) {
        Ok(value) => Ok(value),
        Err(EnvReadError::NotPresent) => Err(Error::new(
            ErrorKind::UndefinedError,
            localization::message(keys::MANIFEST_ENV_MISSING)
                .with_arg("name", name)
                .to_string(),
        )),
        Err(EnvReadError::NotUnicode) => Err(Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::MANIFEST_ENV_INVALID_UTF8)
                .with_arg("name", name)
                .to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    //! Direct tests for the process-backed environment adapter.

    use super::*;

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
