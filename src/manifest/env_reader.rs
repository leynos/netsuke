//! Environment access for the manifest `env()` Jinja helper.
//!
//! Isolated from `manifest::mod` so the reader type, its process-backed
//! default, and the failure mapping sit together, and so neither module
//! exceeds the repository's 400-line file limit.
//!
//! See the "Manifest `env()` reader" section of the developers' guide for this
//! seam's ownership boundary and composition rules.

use std::sync::Arc;

use minijinja::{Error, ErrorKind};

use crate::localization::{self, keys};

/// Environment reader supplied to the `env()` Jinja helper.
///
/// Shared and `Send + Sync` because `minijinja` requires registered functions
/// to be both, and the reader is captured by the registered closure.
pub type EnvReader =
    Arc<dyn Fn(&str) -> std::result::Result<String, std::env::VarError> + Send + Sync>;

/// Reader backed by the live process environment.
///
/// # Examples
///
/// ```rust
/// use netsuke::manifest::process_env_reader;
///
/// let reader = process_env_reader();
/// // Reads whatever the process has; absent variables report `NotPresent`.
/// assert!(reader("NETSUKE_DEFINITELY_UNSET_VARIABLE").is_err());
/// ```
#[must_use]
#[expect(
    clippy::disallowed_methods,
    reason = "composition root: supplies the process environment to the env() Jinja helper"
)]
pub fn process_env_reader() -> EnvReader {
    Arc::new(|key: &str| std::env::var(key))
}

/// Resolve the value of an environment variable for the `env()` Jinja helper.
///
/// Returns the variable's value or a structured error that mirrors Jinja's
/// failure modes, ensuring templates halt when a variable is missing or not
/// valid UTF-8.
/// Resolve `name` through `read_env`, mapping failures to Jinja errors.
///
/// Separated from [`env_var`] so all three outcomes — present, absent, and
/// non-UTF-8 — can be exercised without mutating the process environment. The
/// non-UTF-8 branch is otherwise unreachable from a test: fabricating such a
/// value in the live environment needs platform-specific `OsString` surgery,
/// and the AGENTS.md testing mandate forbids in-process mutation regardless.
///
/// # Examples
///
/// ```rust,ignore
/// let value = env_var_with("FOO", |_| Ok(String::from("bar")));
/// assert_eq!(value.expect("FOO"), "bar");
/// ```
pub(super) fn env_var_with<F>(name: &str, read_env: F) -> std::result::Result<String, Error>
where
    F: FnOnce(&str) -> std::result::Result<String, std::env::VarError>,
{
    match read_env(name) {
        Ok(val) => Ok(val),
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
