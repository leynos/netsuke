//! Environment provider seam for configuration discovery and merging.

use std::ffi::OsString;

/// Provides access to environment variables used during config discovery.
///
/// Production code uses [`StdEnvProvider`]. Tests can provide an in-memory
/// implementation so config-selection logic does not mutate process-global
/// environment state.
pub trait EnvProvider {
    /// Return the value of `key`, or `None` when the key is unset.
    fn get(&self, key: &str) -> Option<OsString>;

    /// Return all values available to the configuration environment layer.
    fn entries(&self) -> Vec<(OsString, OsString)>;
}

/// Environment provider backed by [`std::env::var_os`].
#[derive(Debug, Default, Clone, Copy)]
pub struct StdEnvProvider;

impl EnvProvider for StdEnvProvider {
    #[expect(
        clippy::disallowed_methods,
        reason = "composition root: StdEnvProvider is the process-backed adapter behind the EnvProvider seam"
    )]
    fn get(&self, key: &str) -> Option<OsString> {
        std::env::var_os(key)
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "composition root: StdEnvProvider is the process-backed adapter behind the EnvProvider seam"
    )]
    fn entries(&self) -> Vec<(OsString, OsString)> {
        std::env::vars_os().collect()
    }
}
