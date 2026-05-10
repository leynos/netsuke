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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_underlying_lock_is_held(message: &str) {
        assert!(ENV_LOCK.try_lock().is_err(), "{message}");
    }

    fn assert_underlying_lock_is_released(message: &str) {
        let lock = ENV_LOCK.try_lock().expect(message);
        drop(lock);
    }

    #[test]
    fn reentrant_env_lock_nested_acquire_and_release() {
        {
            let _outer = EnvLock::acquire();
            let _inner = EnvLock::acquire();
        }

        let outer = EnvLock::acquire();
        {
            let _inner = EnvLock::acquire();
            assert_underlying_lock_is_held(
                "ENV_LOCK should remain locked while nested EnvLock guards are alive",
            );
        }

        assert_underlying_lock_is_held(
            "ENV_LOCK should remain locked until the outer EnvLock guard is dropped",
        );

        drop(outer);
        assert_underlying_lock_is_released(
            "ENV_LOCK should be unlocked after final EnvLock guard is dropped",
        );
    }

    #[test]
    fn reentrant_env_lock_stays_locked_when_outer_drops_first() {
        let outer = EnvLock::acquire();
        let inner = EnvLock::acquire();

        drop(outer);
        assert_underlying_lock_is_held(
            "ENV_LOCK should remain locked while an inner EnvLock guard is alive",
        );

        drop(inner);
        assert_underlying_lock_is_released(
            "ENV_LOCK should be unlocked after the final out-of-order guard drops",
        );
    }
}
