//! Override and restore [`NINJA_ENV`] for tests.
//!
//! Provides a helper that sets [`NINJA_ENV`] while ensuring it is restored
//! afterwards. This uses [`EnvLock`] to serialise mutations to the global
//! environment and captures the previous value through a `mockable::Env`
//! implementation so tests can inject their own state.

use mockable::Env;
use netsuke::runner::NINJA_ENV;

/// Guard that resets `NINJA_ENV` on drop.
///
/// Holding the guard keeps the environment override in place. Dropping it
/// restores the prior value, cleaning up global state even if a test panics.
#[derive(Debug)]
pub struct NinjaEnvGuard {
    original: Option<String>,
}

/// Set [`NINJA_ENV`] to `value`, returning a guard that restores the previous
/// value when dropped.
///
/// # Thread Safety
///
/// This function is **not thread-safe**. Callers must hold
/// [`EnvLock`](super::env_lock::EnvLock) to serialise this mutation.
///
/// # Examples
/// ```ignore
/// use mockable::DefaultEnv;
/// use tests::support::ninja_env::override_ninja_env;
/// let env = DefaultEnv::new();
/// let _guard = override_ninja_env(&env, "/usr/bin/ninja");
/// ```
#[allow(dead_code, reason = "only some tests override NINJA_ENV")]
pub fn override_ninja_env(env: &impl Env, value: &str) -> NinjaEnvGuard {
    let original = env.raw(NINJA_ENV).ok();
    // Callers must hold [`EnvLock`](super::env_lock::EnvLock) to serialise this
    // mutation. `set_var` is `unsafe` on Rust 2024 and the guard restores the
    // prior value on drop.
    unsafe { std::env::set_var(NINJA_ENV, value) };
    NinjaEnvGuard { original }
}

impl Drop for NinjaEnvGuard {
    fn drop(&mut self) {
        // Safety: callers hold [`EnvLock`] for the guard's lifetime, so these
        // `set_var`/`remove_var` calls are serialised. Both functions are
        // `unsafe` on Rust 2024.
        unsafe {
            if let Some(ref val) = self.original {
                std::env::set_var(NINJA_ENV, val);
            } else {
                std::env::remove_var(NINJA_ENV);
            }
        }
    }
}
