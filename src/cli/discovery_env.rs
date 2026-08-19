//! Environment access seam for configuration discovery.
//!
//! Kept separate from [`super`]'s discovery module so `discovery.rs` stays
//! within the repository's 400-line cap. Production code uses the
//! process-backed [`StdEnvProvider`]; tests inject an in-memory provider.

use std::ffi::OsString;

/// The environment variable naming Netsuke's explicit configuration file.
pub(super) const CONFIG_ENV_VAR: &str = "NETSUKE_CONFIG";

/// Environment keys consulted during discovery, in lookup priority order.
pub(super) const DISCOVERY_ENV_KEYS: [&str; 7] = [
    CONFIG_ENV_VAR,
    "HOME",
    "USERPROFILE",
    "XDG_CONFIG_HOME",
    "XDG_CONFIG_DIRS",
    "APPDATA",
    "LOCALAPPDATA",
];

/// Provides access to environment variables used during config discovery.
///
/// Production code uses [`StdEnvProvider`]. Tests can provide an in-memory
/// implementation so config-selection logic does not mutate process-global
/// environment state.
pub trait EnvProvider {
    /// Return the value of `key`, or `None` when the key is unset.
    fn get(&self, key: &str) -> Option<OsString>;

    /// Return all values available to the configuration environment layer.
    ///
    /// Providers concerned only with selector lookup may retain the empty
    /// default. Full merge providers override this method.
    fn entries(&self) -> Vec<(OsString, OsString)> {
        Vec::new()
    }
}

/// Environment provider backed by [`std::env::var_os`].
#[derive(Debug, Default, Clone, Copy)]
pub struct StdEnvProvider;

#[expect(
    clippy::disallowed_methods,
    reason = "composition root: StdEnvProvider is the process-backed adapter behind the EnvProvider seam"
)]
impl EnvProvider for StdEnvProvider {
    fn get(&self, key: &str) -> Option<OsString> {
        std::env::var_os(key)
    }

    fn entries(&self) -> Vec<(OsString, OsString)> {
        std::env::vars_os().collect()
    }
}
