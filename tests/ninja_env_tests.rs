//! Tests for overriding the `NINJA_ENV` variable via a mock environment.

use mockable::MockEnv;
use ninja_env::NINJA_ENV;
use rstest::{fixture, rstest};
use serial_test::serial;
use std::path::PathBuf;
use test_support::{env::override_ninja_env, env_lock::EnvLock};

#[fixture]
fn ninja_tmp() -> PathBuf {
    std::env::temp_dir().join("ninja")
}

#[rstest]
#[serial]
fn override_ninja_env_sets_and_restores(ninja_tmp: PathBuf) {
    let before = std::env::var_os(NINJA_ENV);
    let original = before
        .as_ref()
        .and_then(|v| v.to_str().map(ToOwned::to_owned));
    let mut env = MockEnv::new();
    env.expect_raw()
        .withf(|k| k == NINJA_ENV)
        .returning(move |_| original.clone().ok_or(std::env::VarError::NotPresent));
    {
        let _guard = override_ninja_env(&env, ninja_tmp.as_path());
        let after = std::env::var_os(NINJA_ENV).expect("NINJA_ENV should be set after override");
        assert_eq!(after, ninja_tmp.as_os_str());
    }
    let restored = std::env::var_os(NINJA_ENV);
    assert_eq!(restored, before);
}

#[rstest]
#[serial]
fn override_ninja_env_unset_removes_variable(ninja_tmp: PathBuf) {
    let before = std::env::var_os(NINJA_ENV);
    let mut env = MockEnv::new();
    env.expect_raw()
        .withf(|k| k == NINJA_ENV)
        .returning(|_| Err(std::env::VarError::NotPresent));
    {
        let _guard = override_ninja_env(&env, ninja_tmp.as_path());
        let after = std::env::var_os(NINJA_ENV).expect("NINJA_ENV should be set after override");
        assert_eq!(after, ninja_tmp.as_os_str());
    }
    assert!(std::env::var_os(NINJA_ENV).is_none());

    // Restore original global state for isolation
    {
        let _lock = EnvLock::acquire();
        if let Some(val) = before {
            // EnvLock serialises mutations while restoring.
            unsafe { std::env::set_var(NINJA_ENV, val) };
        } else {
            // EnvLock serialises mutations while restoring.
            unsafe { std::env::remove_var(NINJA_ENV) };
        }
    }
}
