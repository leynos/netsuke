//! Helpers for managing environment variable mutations in BDD tests.
//!
//! Provides shared utilities for safely mutating process-global environment
//! variables within BDD scenarios using the `EnvLock` serialisation mechanism.

use crate::bdd::fixtures::TestWorld;
use crate::bdd::types::EnvVarKey;
use anyhow::{Result, ensure};

/// Mutate an environment variable under the scenario's `EnvLock`, track it for
/// cleanup, and return `Ok(())`.
///
/// Acquires the scenario-scoped `EnvLock` via `world.ensure_env_lock()` to
/// serialise all process-global mutations (environment variables and CWD),
/// then performs the mutation and registers the key for cleanup at scenario end.
///
/// # Parameters
///
/// - `world`: The `TestWorld` scenario context.
/// - `key`: The environment variable name.
/// - `new_value`:
///   - `Some(s)` – set the variable to `s`.
///   - `None`    – remove the variable.
///
/// # Errors
///
/// Returns an error if the environment variable name is empty.
pub fn mutate_env_var(world: &TestWorld, key: EnvVarKey, new_value: Option<&str>) -> Result<()> {
    ensure!(
        !key.as_str().is_empty(),
        "environment variable name must not be empty"
    );
    world.ensure_env_lock();
    let original = std::env::var_os(key.as_str());
    // SAFETY: EnvLock (held via world.env_lock) serialises mutations
    unsafe {
        match new_value {
            Some(val) => std::env::set_var(key.as_str(), val),
            None => std::env::remove_var(key.as_str()),
        }
    }
    world.track_env_var(key.into_string(), original);
    Ok(())
}
