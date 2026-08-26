//! Retain the retiring `EnvLock` legacy seam during caller migration.
//!
//! `EnvLock` remains only until its callers migrate to injected
//! `mockable::Env` seams and issue #494 removes it. Add no callers or tests;
//! [ADR-008](../../docs/adr-008-environment-seam-taxonomy.md) records the
//! retirement decision.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard};
use std::{fmt, fmt::Formatter};

/// Global mutex serializing all environment mutations.
static ENV_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    static ENV_LOCK_STATE: RefCell<LockState> = const { RefCell::new(LockState {
        depth: 0,
        guard: None,
    }) };
}

/// Per-thread bookkeeping for the global environment lock.
struct LockState {
    /// Number of live `EnvLock` guards on this thread.
    depth: usize,
    /// The held mutex guard, present only while `depth` is non-zero.
    guard: Option<MutexGuard<'static, ()>>,
}

/// RAII guard that holds the global environment lock.
///
/// The guard is thread-bound because its underlying mutex guard is stored in
/// thread-local state. Moving it to another thread would leave the acquiring
/// thread's lock depth and guard out of sync.
///
/// ```compile_fail
/// use test_support::env_lock::EnvLock;
///
/// let guard = EnvLock::acquire();
/// std::thread::spawn(move || drop(guard));
/// ```
pub struct EnvLock {
    /// Marker making the guard `!Send`, keeping the lock thread-bound.
    _not_send: PhantomData<Rc<()>>,
}

impl fmt::Debug for EnvLock {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnvLock").finish_non_exhaustive()
    }
}

impl EnvLock {
    /// Acquire the global lock serializing environment mutations.
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
        Self {
            _not_send: PhantomData,
        }
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
    //! Unit tests for reentrant and contended environment locking.

    use super::*;
    use proptest::prelude::{Just, Strategy, prop_oneof};
    use std::{sync::mpsc, time::Duration};

    /// Read the thread-local lock state without asserting.
    ///
    /// Callers holding a live [`EnvLock`] need this: asserting while the guard
    /// is alive would drop it during unwind, and dropping a `MutexGuard` while
    /// panicking poisons the mutex.
    fn current_thread_lock_is_held() -> bool {
        ENV_LOCK_STATE.with(|lock_state| {
            let state = lock_state.borrow();
            state.depth > 0 && state.guard.is_some()
        })
    }

    fn assert_current_thread_lock_is_released(message: &str) {
        ENV_LOCK_STATE.with(|lock_state| {
            let state = lock_state.borrow();
            assert!(state.depth == 0 && state.guard.is_none(), "{message}");
        });
    }

    #[test]
    fn reentrant_env_lock_nested_acquire_and_release() {
        {
            let _outer = EnvLock::acquire();
            let _inner = EnvLock::acquire();
        }

        // Observations are captured while the guards live and asserted only once
        // they are released: a failing assertion under a live guard would drop it
        // mid-unwind and poison `ENV_LOCK`, burying the real cause.
        let outer = EnvLock::acquire();
        let held_with_nested = {
            let _inner = EnvLock::acquire();
            current_thread_lock_is_held()
        };
        let held_after_nested_drop = current_thread_lock_is_held();

        drop(outer);
        assert!(
            held_with_nested,
            "ENV_LOCK should remain locked while nested EnvLock guards are alive"
        );
        assert!(
            held_after_nested_drop,
            "ENV_LOCK should remain locked until the outer EnvLock guard is dropped"
        );
        assert_current_thread_lock_is_released(
            "ENV_LOCK should be unlocked after final EnvLock guard is dropped",
        );
    }

    #[test]
    fn reentrant_env_lock_stays_locked_when_outer_drops_first() {
        let outer = EnvLock::acquire();
        let inner = EnvLock::acquire();

        drop(outer);
        let held_with_inner_alive = current_thread_lock_is_held();

        drop(inner);
        assert!(
            held_with_inner_alive,
            "ENV_LOCK should remain locked while an inner EnvLock guard is alive"
        );
        assert_current_thread_lock_is_released(
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

    /// One step of the generated transition sequence.
    #[derive(Clone, Copy, Debug)]
    enum LockOp {
        Acquire,
        /// Drop the guard at `selector`, clamped to the live range, covering
        /// nested and out-of-order release; ignored while no guard is live.
        Drop(usize),
    }

    fn assert_state_matches(live_guards: usize) {
        ENV_LOCK_STATE.with(|lock_state| {
            let state = lock_state.borrow();
            assert_eq!(
                state.depth, live_guards,
                "thread-local depth must equal the number of live guards"
            );
            assert_eq!(
                state.guard.is_some(),
                live_guards > 0,
                "the mutex guard must be held exactly while guards are live"
            );
        });
    }

    proptest::proptest! {
        /// Any bounded interleaving of acquires and arbitrary-index drops
        /// keeps the thread-local depth equal to the number of live guards,
        /// and holds the underlying mutex exactly while that count is
        /// non-zero.
        #[test]
        fn lock_state_tracks_any_acquire_and_drop_interleaving(
            ops in proptest::collection::vec(
                prop_oneof![
                    Just(LockOp::Acquire),
                    (0usize..8).prop_map(LockOp::Drop),
                ],
                1..24,
            ),
        ) {
            let mut live: Vec<EnvLock> = Vec::new();
            for op in ops {
                match op {
                    LockOp::Acquire => live.push(EnvLock::acquire()),
                    LockOp::Drop(selector) => {
                        if let Some(last) = live.len().checked_sub(1) {
                            drop(live.remove(selector.min(last)));
                        }
                    }
                }
                assert_state_matches(live.len());
            }

            while let Some(guard) = live.pop() {
                drop(guard);
                assert_state_matches(live.len());
            }
            assert_current_thread_lock_is_released(
                "dropping every generated guard must release the lock",
            );
        }
    }

    /// Probes `ENV_LOCK` directly, so it requires per-test process isolation
    /// (the suite runs under `cargo nextest`, which forks each test). Under a
    /// thread-parallel runner it would race any other test touching the global
    /// mutex.
    ///
    /// No assertion here can leave process-global poisoning behind, though the
    /// reason differs either side of `ENV_LOCK.clear_poison()`:
    ///
    /// - The `join` assertion runs before the flag is cleared, so `ENV_LOCK` is
    ///   still poisoned at that point — deliberately, since that is the state
    ///   under test. It is safe regardless: the poisoned guard belonged to the
    ///   spawned thread, and this thread holds no `EnvLock`, so a failure has no
    ///   live guard to drop.
    /// - The later assertions run once the flag is cleared, and the held state is
    ///   captured before its guard is dropped, so an unwinding assertion cannot
    ///   drop a live `MutexGuard` and poison the mutex afresh.
    #[test]
    fn env_lock_recovers_after_mutex_poisoning() {
        let poisoner = std::thread::spawn(|| {
            let _guard = EnvLock::acquire();
            panic!("poison ENV_LOCK deliberately");
        });

        assert!(poisoner.join().is_err(), "poisoning thread should panic");
        let was_poisoned = ENV_LOCK.is_poisoned();
        ENV_LOCK.clear_poison();
        assert!(was_poisoned, "the panic should poison the underlying mutex");

        let recovered_guard = EnvLock::acquire();
        let held_while_live = current_thread_lock_is_held();
        drop(recovered_guard);
        assert!(
            held_while_live,
            "recovered EnvLock guard should hold ENV_LOCK"
        );
        assert_current_thread_lock_is_released("recovered ENV_LOCK should be released normally");
    }
}
