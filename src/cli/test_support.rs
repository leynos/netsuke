//! Mockable environment builders for CLI configuration tests.
//!
//! The helpers return [`mockable::MockEnv`] instances, so tests inject each
//! environment read rather than mutating the process environment.

use mockable::MockEnv;
use std::{collections::HashMap, ffi::OsString};

/// Build a `MockEnv` that returns `entries` for `os_string` lookups.
pub(crate) fn mock_env_with<K, V>(entries: impl IntoIterator<Item = (K, V)>) -> MockEnv
where
    K: AsRef<str>,
    V: Into<OsString>,
{
    let values = entries
        .into_iter()
        .map(|(key, value)| (key.as_ref().to_owned(), value.into()))
        .collect::<HashMap<_, _>>();
    let mut env = MockEnv::new();
    env.expect_os_string()
        .returning(move |key| values.get(key).cloned());
    env
}

/// Build a `MockEnv` with no configured `os_string` values.
pub(crate) fn empty_mock_env() -> MockEnv {
    mock_env_with(std::iter::empty::<(&str, OsString)>())
}
