//! Tests for overriding the NINJA_ENV variable via a mock environment.

use mockable::MockEnv;
use netsuke::runner::NINJA_ENV;
use rstest::rstest;
use serial_test::serial;
use std::path::Path;
use test_support::{env::override_ninja_env, env_lock::EnvLock};

#[rstest]
#[serial]
fn override_ninja_env_sets_and_restores() {
    let before = std::env::var_os(NINJA_ENV);
    let mut env = MockEnv::new();
    env.expect_raw()
        .withf(|k| k == NINJA_ENV)
        .returning(|_| Ok("restored".to_string()));
    {
        let guard = override_ninja_env(&env, Path::new("/tmp/ninja"));
        let after = std::env::var(NINJA_ENV).expect("env var");
        assert_eq!(after, "/tmp/ninja");
        drop(guard);
    }
    let restored = std::env::var(NINJA_ENV).expect("env var");
    assert_eq!(restored, "restored");
    let _lock = EnvLock::acquire();
    // SAFETY: `EnvLock` serialises access while the variable is reset.
    unsafe {
        if let Some(val) = before {
            std::env::set_var(NINJA_ENV, val);
        } else {
            std::env::remove_var(NINJA_ENV);
        }
    }
}
