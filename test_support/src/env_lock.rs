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
    //! Unit tests for the environment mutation lock.

    use super::*;
    use rstest::{fixture, rstest};
    use std::sync::PoisonError;
    use std::thread;

    /// Serialises this module's tests against each other.
    ///
    /// Only the poisoning test needs this: it sets and clears the sticky,
    /// process-global poison flag, and two tests doing that concurrently would
    /// observe each other. The held/released assertions below are thread-local
    /// and need no serialisation. nextest isolates every test in its own
    /// process and never sees any of this, but plain `cargo test` shares one,
    /// and the crate must not be flaky under either.
    static TEST_SERIAL: Mutex<()> = Mutex::new(());

    /// Hold the module's serialisation lock for the whole test.
    ///
    /// A fixture rather than a `let` binding so the guard is a parameter, and
    /// so drops after every local the test declares.
    ///
    /// Recovers from poisoning: a panicking test leaves the flag set, and that
    /// must not cascade into every later test in the module.
    #[fixture]
    fn serialised() -> MutexGuard<'static, ()> {
        TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
    }

    // Macros rather than helper functions so a failure reports the calling
    // test's line number.
    //
    // These probe this thread's `ENV_LOCK_STATE`, not `ENV_LOCK.try_lock()`.
    // The global probe was racy and did fail in practice: `ENV_LOCK` is
    // acquired by `env::set_var`, `env::with_isolated_path`,
    // `EnvGuard::drop` and other *library* functions, so any test anywhere
    // calling the public API holds it transitively. A concurrent holder made
    // `try_lock` return `WouldBlock`, and the released-assertion reported
    // "ENV_LOCK is still held" for a guard this thread had correctly dropped.
    // Serialising this module's tests could not fix that, because the
    // competing acquisitions are not in this module and not in tests at all.
    //
    // The thread-local probe is both race-free and more precise. Holding a
    // `MutexGuard` means the mutex is locked, so `guard.is_some()` is exactly
    // "this thread holds `ENV_LOCK`" and `guard.is_none()` is exactly "this
    // thread released it" — which is the `Drop` contract under test. Another
    // thread's unrelated guard is now correctly invisible.
    macro_rules! assert_underlying_lock_is_held {
        ($message:expr $(,)?) => {
            ENV_LOCK_STATE.with(|state| {
                assert!(
                    state.borrow().guard.is_some(),
                    "{}: this thread holds no ENV_LOCK guard",
                    $message
                );
            });
        };
    }

    macro_rules! assert_underlying_lock_is_released {
        ($message:expr $(,)?) => {
            ENV_LOCK_STATE.with(|state| {
                assert!(
                    state.borrow().guard.is_none(),
                    "{}: this thread still holds an ENV_LOCK guard",
                    $message
                );
            });
        };
    }

    #[rstest]
    fn reentrant_env_lock_nested_acquire_and_release(
        #[from(serialised)] _serial: MutexGuard<'static, ()>,
    ) {
        {
            let _outer = EnvLock::acquire();
            let _inner = EnvLock::acquire();
        }

        let outer = EnvLock::acquire();
        {
            let _inner = EnvLock::acquire();
            assert_underlying_lock_is_held!(
                "ENV_LOCK should remain locked while nested EnvLock guards are alive",
            );
        }

        assert_underlying_lock_is_held!(
            "ENV_LOCK should remain locked until the outer EnvLock guard is dropped",
        );

        drop(outer);
        assert_underlying_lock_is_released!(
            "ENV_LOCK should be unlocked after final EnvLock guard is dropped",
        );
    }

    /// Poison `ENV_LOCK` the way a panicking test would: hold the guard across
    /// a panic on another thread.
    ///
    /// The panic hook is left alone. `panic::set_hook` is process-wide, so
    /// swapping it out would suppress the report any concurrently panicking
    /// test relies on. One line of stderr noise is the cheaper cost.
    fn poison_env_lock() {
        let poisoner = thread::spawn(|| {
            let _guard = ENV_LOCK.lock();
            panic!("deliberately poisoning ENV_LOCK");
        });
        assert!(
            poisoner.join().is_err(),
            "the poisoning thread should have panicked"
        );
        assert!(ENV_LOCK.is_poisoned(), "ENV_LOCK should now be poisoned");
    }

    #[rstest]
    fn env_lock_recovers_from_a_poisoned_mutex(
        #[from(serialised)] _serial: MutexGuard<'static, ()>,
    ) {
        poison_env_lock();

        // `acquire` must recover through `PoisonError::into_inner` rather than
        // propagating: a panic here would fail every later test taking the lock.
        let guard = EnvLock::acquire();
        assert_underlying_lock_is_held!(
            "a recovered EnvLock should still hold the underlying mutex",
        );

        drop(guard);
        assert_underlying_lock_is_released!(
            "a recovered EnvLock should release the mutex when dropped",
        );

        // Leave the static as it was found. The flag is sticky and the mutex is
        // shared with every other test, so a deliberate poisoning must not
        // outlive the test that caused it.
        ENV_LOCK.clear_poison();
        assert!(
            !ENV_LOCK.is_poisoned(),
            "the poison flag should be cleared before leaving the test"
        );
    }

    #[rstest]
    fn reentrant_env_lock_stays_locked_when_outer_drops_first(
        #[from(serialised)] _serial: MutexGuard<'static, ()>,
    ) {
        let outer = EnvLock::acquire();
        let inner = EnvLock::acquire();

        drop(outer);
        assert_underlying_lock_is_held!(
            "ENV_LOCK should remain locked while an inner EnvLock guard is alive",
        );

        drop(inner);
        assert_underlying_lock_is_released!(
            "ENV_LOCK should be unlocked after the final out-of-order guard drops",
        );
    }
}
