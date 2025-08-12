//! Serialise environment mutations across tests.
//!
//! The `EnvLock` guard ensures that changes to global state like `PATH` are
//! synchronised, preventing interference between concurrently running tests.

use std::sync::{Mutex, MutexGuard};

/// Global mutex protecting environment changes.
#[cfg_attr(test, expect(dead_code, reason = "only some tests mutate PATH"))]
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// RAII guard that holds the global environment lock.
#[cfg_attr(not(test), expect(dead_code, reason = "only some tests mutate PATH"))]
pub struct EnvLock(MutexGuard<'static, ()>);

impl EnvLock {
    /// Acquire the global lock serialising environment mutations.
    #[cfg_attr(not(test), expect(dead_code, reason = "only some tests mutate PATH"))]
    pub fn acquire() -> Self {
        Self(ENV_LOCK.lock().expect("env lock"))
    }
}
