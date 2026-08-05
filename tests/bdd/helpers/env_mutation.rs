//! Helpers for composing child-process environments in BDD tests.
//!
//! Scenario variables are recorded in the world and applied only to spawned
//! Netsuke processes.

use crate::bdd::fixtures::TestWorld;
use crate::bdd::types::EnvVarKey;
use anyhow::{Result, ensure};
use std::ffi::OsString;

/// Record an environment variable for the scenario's child processes.
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
/// Returns an error if the environment variable name is empty, contains '=',
/// or contains `'\0'`, or if `new_value` contains `'\0'`.
pub fn mutate_env_var(world: &TestWorld, key: EnvVarKey, new_value: Option<&str>) -> Result<()> {
    ensure!(
        !key.as_str().is_empty(),
        "environment variable name must not be empty"
    );
    ensure!(
        !key.as_str().contains('='),
        "environment variable name must not contain '='"
    );
    ensure!(
        !key.as_str().contains('\0'),
        "environment variable name must not contain null bytes"
    );
    if let Some(val) = new_value {
        ensure!(
            !val.contains('\0'),
            "environment variable value must not contain null bytes"
        );
    }
    let forward_value = new_value.map(OsString::from);
    world.track_env_var(key.into_string(), forward_value);
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Unit tests for environment-mutation helpers.
    use super::*;
    use crate::bdd::fixtures::TestWorld;
    use crate::bdd::types::EnvVarKey;
    use rstest::{fixture, rstest};

    #[fixture]
    fn test_world() -> TestWorld {
        TestWorld::default()
    }

    struct MutationTestCase {
        key: &'static str,
        new_value: Option<&'static str>,
        expect_error: bool,
        expect_present: bool,
    }

    #[rstest]
    #[case::empty_key(MutationTestCase { key: "", new_value: None, expect_error: true, expect_present: false })]
    #[case::key_with_equals(MutationTestCase { key: "KEY=VALUE", new_value: Some("test"), expect_error: true, expect_present: false })]
    #[case::key_with_null(MutationTestCase { key: "KEY\0NULL", new_value: Some("test"), expect_error: true, expect_present: false })]
    #[case::value_with_null(MutationTestCase { key: "NETSUKE_TEST_MUTATE_ENV_VAR_VALUE_NULL", new_value: Some("bad\0value"), expect_error: true, expect_present: false })]
    #[case::set_new_var(MutationTestCase { key: "NETSUKE_TEST_MUTATE_ENV_VAR_SET", new_value: Some("sentinel"), expect_error: false, expect_present: true })]
    #[case::remove_existing_var(MutationTestCase { key: "NETSUKE_TEST_MUTATE_ENV_VAR_REMOVE", new_value: None, expect_error: false, expect_present: false })]
    fn mutate_env_var_handles_various_operations(
        test_world: TestWorld,
        #[case] tc: MutationTestCase,
    ) {
        // For the set case, ensure variable is absent first
        if tc.key == "NETSUKE_TEST_MUTATE_ENV_VAR_SET" {
            mutate_env_var(&test_world, EnvVarKey::new(tc.key), None)
                .expect("precondition cleanup should succeed");
        }

        // For the remove case, seed the variable first
        if tc.key == "NETSUKE_TEST_MUTATE_ENV_VAR_REMOVE" {
            mutate_env_var(&test_world, EnvVarKey::new(tc.key), Some("present"))
                .expect("seed should succeed");
        }

        // Perform the operation under test
        let result = mutate_env_var(&test_world, EnvVarKey::new(tc.key), tc.new_value);

        if tc.expect_error {
            assert!(
                result.is_err(),
                "invalid key or value should be rejected before mutating the environment"
            );
        } else {
            assert!(result.is_ok(), "operation should succeed");

            if tc.expect_present {
                let recorded = test_world.env_vars_forward.borrow();
                assert_eq!(
                    recorded.get(tc.key).and_then(|value| value.to_str()),
                    tc.new_value,
                    "variable should be recorded with the expected value"
                );
                drop(recorded);
                // Cleanup
                mutate_env_var(&test_world, EnvVarKey::new(tc.key), None)
                    .expect("cleanup should succeed");
            } else if !tc.key.is_empty() {
                assert!(
                    !test_world.env_vars_forward.borrow().contains_key(tc.key),
                    "variable should be absent from the child environment"
                );
            }
        }
    }
}
