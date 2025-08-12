//! Serialise environment mutations across tests.
//!
//! The `EnvLock` guard ensures that changes to global state like `PATH` are
//! synchronised, preventing interference between concurrently running tests.

use std::sync::{Mutex, MutexGuard};

#[allow(dead_code, reason = "only some tests mutate PATH")]
/// Global mutex protecting environment changes.
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[allow(dead_code, reason = "only some tests mutate PATH")]
/// RAII guard that holds the global environment lock.
pub struct EnvLock(MutexGuard<'static, ()>);

impl EnvLock {
    #[allow(dead_code, reason = "only some tests mutate PATH")]
    /// Acquire the global lock serialising environment mutations.
    pub fn acquire() -> Self {
        Self(ENV_LOCK.lock().expect("env lock"))
    }
}
