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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bdd::fixtures::TestWorld;
    use crate::bdd::types::EnvVarKey;

    #[test]
    fn mutate_env_var_returns_error_for_empty_key() {
        let world = TestWorld::default();
        let result = mutate_env_var(&world, EnvVarKey::new(""), Some("value"));
        assert!(result.is_err(), "empty key should be rejected");
    }

    #[test]
    fn mutate_env_var_sets_and_tracks_variable() {
        let world = TestWorld::default();
        let key = "NETSUKE_TEST_MUTATE_ENV_VAR_SET";
        // Ensure the variable is absent before the test
        unsafe { std::env::remove_var(key) };
        mutate_env_var(&world, EnvVarKey::new(key), Some("sentinel")).expect("set should succeed");
        assert_eq!(
            std::env::var(key).ok().as_deref(),
            Some("sentinel"),
            "variable should be set"
        );
        // Cleanup
        unsafe { std::env::remove_var(key) };
    }

    #[test]
    fn mutate_env_var_removes_variable_when_new_value_is_none() {
        let world = TestWorld::default();
        let key = "NETSUKE_TEST_MUTATE_ENV_VAR_REMOVE";
        unsafe { std::env::set_var(key, "present") };
        mutate_env_var(&world, EnvVarKey::new(key), None).expect("remove should succeed");
        assert!(
            std::env::var(key).is_err(),
            "variable should have been removed"
        );
    }
}
