//! Serialise environment mutations across tests.
//!
//! The `EnvLock` guard ensures that changes to global state like `PATH` are
//! synchronised, preventing interference between concurrently running tests.

use std::cell::RefCell;
use std::sync::{Mutex, MutexGuard};
use std::{fmt, fmt::Formatter};

static ENV_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    static ENV_LOCK_STATE: RefCell<LockState> = const { RefCell::new(LockState {
        depth: 0,
        guard: None,
    }) };
}

struct LockState {
    depth: usize,
    guard: Option<MutexGuard<'static, ()>>,
}

/// RAII guard that holds the global environment lock.
pub struct EnvLock {
    _private: (),
}

impl fmt::Debug for EnvLock {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnvLock").finish_non_exhaustive()
    }
}

impl EnvLock {
    /// Acquire the global lock serialising environment mutations.
    pub fn acquire() -> Self {
        ENV_LOCK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            if state.depth == 0 {
                state.guard = Some(
                    ENV_LOCK
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner()),
                );
            }
            state.depth += 1;
        });
        Self { _private: () }
    }
}

impl Drop for EnvLock {
    fn drop(&mut self) {
        ENV_LOCK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.depth = state.depth.saturating_sub(1);
            if state.depth == 0 {
                drop(state.guard.take());
            }
        });
    }
}
