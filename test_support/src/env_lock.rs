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
    #[must_use]
    pub fn acquire() -> Self {
        ENV_LOCK_STATE.with(|lock_state| {
            let mut state_ref = lock_state.borrow_mut();
            if state_ref.depth == 0 {
                state_ref.guard = Some(
                    ENV_LOCK
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner),
                );
            }
            state_ref.depth += 1;
        });
        Self { _private: () }
    }
}

impl Drop for EnvLock {
    fn drop(&mut self) {
        ENV_LOCK_STATE.with(|lock_state| {
            let mut state_ref = lock_state.borrow_mut();
            state_ref.depth = state_ref.depth.saturating_sub(1);
            if state_ref.depth == 0 {
                drop(state_ref.guard.take());
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

    fn assert_underlying_lock_is_held(message: &str) {
        assert!(ENV_LOCK.try_lock().is_err(), "{message}");
    }

    fn assert_underlying_lock_is_released(message: &str) {
        let Ok(lock) = ENV_LOCK.try_lock() else {
            panic!("{message}");
        };
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

    #[test]
    fn env_lock_serializes_contending_threads_until_release() {
        let outer = EnvLock::acquire();
        let (attempting_tx, attempting_rx) = mpsc::channel();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let contender = std::thread::spawn(move || {
            assert!(
                attempting_tx.send(()).is_ok(),
                "main test thread should await the contention attempt"
            );
            let _guard = EnvLock::acquire();
            assert!(
                acquired_tx.send(()).is_ok(),
                "main test thread should await lock acquisition"
            );
        });

        assert!(
            attempting_rx.recv_timeout(Duration::from_secs(1)).is_ok(),
            "contending thread should attempt lock acquisition"
        );
        assert_eq!(
            acquired_rx.recv_timeout(Duration::from_millis(20)),
            Err(mpsc::RecvTimeoutError::Timeout),
            "contending thread must remain blocked while the outer guard lives"
        );

        drop(outer);
        assert!(
            acquired_rx.recv_timeout(Duration::from_secs(1)).is_ok(),
            "contending thread should acquire the lock after release"
        );
        assert!(contender.join().is_ok(), "contending thread should finish");
    }
}
