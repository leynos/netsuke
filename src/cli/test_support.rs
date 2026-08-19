//! Shared test doubles for CLI configuration tests.
//!
//! `TestEnv` provides deterministic, in-memory environment values for unit
//! tests that exercise [`super::discovery::EnvProvider`] without mutating the
//! process environment.

use std::{collections::HashMap, ffi::OsString};

use super::discovery::EnvProvider;

/// In-memory environment values for deterministic CLI configuration tests.
#[derive(Default)]
pub(crate) struct TestEnv {
    values: HashMap<&'static str, OsString>,
}

impl TestEnv {
    /// Add one environment value to this test double.
    pub(crate) fn with_var(mut self, name: &'static str, value: impl Into<OsString>) -> Self {
        self.values.insert(name, value.into());
        self
    }
}

impl EnvProvider for TestEnv {
    fn get(&self, key: &str) -> Option<OsString> {
        self.values.get(key).cloned()
    }
}
