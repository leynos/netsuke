//! Environment access for the manifest `env()` Jinja helper.
//!
//! The reader port keeps process access at the composition root while tests
//! inject deterministic values without mutating global state.

use std::sync::Arc;

use minijinja::{Error, ErrorKind};
use mockable::{DefaultEnv, Env};

use crate::localization::{self, keys};

/// Environment reader supplied to the `env()` Jinja helper.
pub type EnvReader =
    Arc<dyn Fn(&str) -> std::result::Result<String, std::env::VarError> + Send + Sync>;

/// Construct the process-backed environment reader used by production loads.
#[must_use]
pub fn process_env_reader() -> EnvReader {
    let env = DefaultEnv;
    Arc::new(move |key| env.raw(key))
}

/// Resolve `name` through `read_env`, mapping failures to Jinja errors.
pub(super) fn env_var_with(
    name: &str,
    read_env: impl FnOnce(&str) -> std::result::Result<String, std::env::VarError>,
) -> std::result::Result<String, Error> {
    match read_env(name) {
        Ok(value) => Ok(value),
        Err(std::env::VarError::NotPresent) => Err(Error::new(
            ErrorKind::UndefinedError,
            localization::message(keys::MANIFEST_ENV_MISSING)
                .with_arg("name", name)
                .to_string(),
        )),
        Err(std::env::VarError::NotUnicode(_)) => Err(Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::MANIFEST_ENV_INVALID_UTF8)
                .with_arg("name", name)
                .to_string(),
        )),
    }
}
