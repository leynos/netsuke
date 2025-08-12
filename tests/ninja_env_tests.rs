use mockable::MockEnv;
use netsuke::runner::NINJA_ENV;
use std::env::VarError;
use support::env_lock::EnvLock;
use support::ninja_env::override_ninja_env;

#[expect(
    unused,
    reason = "support module exports helpers unused in these tests"
)]
mod support;

#[test]
fn override_ninja_env_restores_original() {
    let mut env = MockEnv::new();
    env.expect_raw()
        .withf(|k| k == NINJA_ENV)
        .times(1)
        .returning(|_| Ok("orig".to_string()));

    {
        let _guard = override_ninja_env(EnvLock::acquire(), &env, "new");
        assert_eq!(std::env::var(NINJA_ENV).as_deref(), Ok("new"));
    }
    assert_eq!(std::env::var(NINJA_ENV).as_deref(), Ok("orig"));
    // Clean up to avoid leaking environment state. `remove_var` is `unsafe`
    // on Rust 2024; a fresh lock serialises this mutation.
    let _cleanup = EnvLock::acquire();
    unsafe { std::env::remove_var(NINJA_ENV) };
}

#[test]
fn override_ninja_env_removes_when_unset() {
    let mut env = MockEnv::new();
    env.expect_raw()
        .withf(|k| k == NINJA_ENV)
        .times(1)
        .returning(|_| Err(VarError::NotPresent));

    {
        let _guard = override_ninja_env(EnvLock::acquire(), &env, "new");
        assert_eq!(std::env::var(NINJA_ENV).as_deref(), Ok("new"));
    }
    assert!(std::env::var(NINJA_ENV).is_err());
    let _cleanup = EnvLock::acquire();
    unsafe { std::env::remove_var(NINJA_ENV) };
}

#[test]
fn override_ninja_env_restores_empty() {
    let mut env = MockEnv::new();
    env.expect_raw()
        .withf(|k| k == NINJA_ENV)
        .times(1)
        .returning(|_| Ok(String::new()));

    {
        let _guard = override_ninja_env(EnvLock::acquire(), &env, "new");
        assert_eq!(std::env::var(NINJA_ENV).as_deref(), Ok("new"));
    }
    assert_eq!(std::env::var(NINJA_ENV).as_deref(), Ok(""));
    let _cleanup = EnvLock::acquire();
    unsafe { std::env::remove_var(NINJA_ENV) };
}
