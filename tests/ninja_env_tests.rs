use mockable::MockEnv;
use netsuke::runner::NINJA_ENV;
use rstest::rstest;
use std::env::VarError;
use support::env_lock::EnvLock;
use support::ninja_env::override_ninja_env;

#[expect(
    unused,
    reason = "support module exports helpers unused in these tests"
)]
mod support;

#[rstest]
#[case(Some("orig"))]
#[case(None)]
#[case(Some(""))]
fn override_ninja_env_restores(#[case] original: Option<&'static str>) {
    let mut env = MockEnv::new();
    match original {
        Some(val) => {
            let returned = val.to_string();
            env.expect_raw()
                .withf(|k| k == NINJA_ENV)
                .times(1)
                .return_once(move |_| Ok(returned));
        }
        None => {
            env.expect_raw()
                .withf(|k| k == NINJA_ENV)
                .times(1)
                .return_once(|_| Err(VarError::NotPresent));
        }
    }

    {
        let _guard = override_ninja_env(EnvLock::acquire(), &env, "new");
        assert_eq!(std::env::var(NINJA_ENV).as_deref(), Ok("new"));
    }

    match original {
        Some(val) => assert_eq!(std::env::var(NINJA_ENV).as_deref(), Ok(val)),
        None => assert!(std::env::var(NINJA_ENV).is_err()),
    }

    let _cleanup = EnvLock::acquire();
    // SAFETY: `EnvLock` serialises this mutation; see above for details.
    unsafe { std::env::remove_var(NINJA_ENV) };
}
