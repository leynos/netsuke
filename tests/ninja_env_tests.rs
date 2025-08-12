use mockable::MockEnv;
use netsuke::runner::NINJA_ENV;
use support::env_lock::EnvLock;
use support::ninja_env::override_ninja_env;

mod support;

#[test]
fn override_ninja_env_restores_original() {
    let _lock = EnvLock::acquire();
    let mut env = MockEnv::new();
    env.expect_raw()
        .withf(|k| k == NINJA_ENV)
        .returning(|_| Ok("orig".to_string()));

    {
        let _guard = override_ninja_env(&env, "new");
        assert_eq!(std::env::var(NINJA_ENV).as_deref(), Ok("new"));
    }
    assert_eq!(std::env::var(NINJA_ENV).as_deref(), Ok("orig"));
    // Clean up to avoid leaking environment state. `remove_var` is `unsafe`
    // on Rust 2024; the lock above serialises this mutation.
    unsafe { std::env::remove_var(NINJA_ENV) };
}
