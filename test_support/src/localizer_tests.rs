//! Tests for the localizer guards' drop ordering.
//!
//! The invariant under test is not observable from a single thread: it only
//! matters when a second test is waiting for the lock. These drive that
//! contention deterministically rather than by timing.

use super::*;
use netsuke::localization::{self, keys};
use std::sync::mpsc;
use std::sync::{Barrier, TryLockError};
use std::thread;

/// Whether the globally installed localizer is currently the French one.
///
/// Identity is not observable through `Arc<dyn Localizer>`, so the rendered
/// text stands in for it: `cli.about` differs between French and the English
/// source.
fn localizer_is_french() -> bool {
    localization::message(keys::CLI_ABOUT)
        .to_string()
        .contains("manifestes")
}

/// The lock must not be released before the previous localizer is restored.
///
/// A waiting thread is admitted only once the whole guard has dropped, so the
/// localizer it observes is the restored one. Were the fields declared the
/// other way round, the lock would be released first and the waiting thread
/// could be admitted into a window where the French localizer is still
/// installed.
///
/// The barrier makes the waiter demonstrably contended before the drop begins:
/// the main thread holds the lock across it, so the waiter's `lock()` call is
/// blocked, not merely late.
///
/// What the waiter sees is compared against the state captured *before* the
/// guard was installed, not against "not French". Restoration means putting
/// back whatever was there, and asserting a particular value instead would
/// make this test fail for a reason of its own if the process default ever
/// changed.
#[test]
fn the_lock_is_held_until_the_localizer_is_restored() {
    let french_before = localizer_is_french();
    let bundle = locale_localizer("fr");
    assert!(
        localizer_is_french(),
        "the fixture must install the French localizer"
    );

    let barrier = Arc::new(Barrier::new(2));
    let (tx, rx) = mpsc::channel();
    let waiter_barrier = Arc::clone(&barrier);
    let waiter = thread::spawn(move || {
        // Signal readiness, then block on the lock the main thread still holds.
        waiter_barrier.wait();
        let guard = localizer_test_lock();
        let observed_french = localizer_is_french();
        drop(guard);
        // The receiver outlives this send; a closed channel would fail the
        // test at `recv` rather than here.
        tx.send(observed_french).ok();
    });

    // Released only after the waiter is running and about to contend.
    barrier.wait();
    // The waiter cannot hold the lock: this thread does, through `bundle`.
    assert!(
        matches!(
            LOCALIZER_TEST_LOCK
                .get()
                .expect("the lock is initialised by the fixture")
                .try_lock(),
            Err(TryLockError::WouldBlock)
        ),
        "the fixture must still hold the lock before it is dropped"
    );

    drop(bundle);

    let observed_french = rx.recv().expect("the waiting thread reports what it saw");
    waiter.join().expect("the waiting thread completes");
    assert_eq!(
        observed_french, french_before,
        "a thread admitted after the drop must see the restored localizer, \
         not the one the guard installed"
    );
}

/// The guard restores the previous localizer when it drops.
///
/// Sequential, not nested: `LOCALIZER_TEST_LOCK` is a plain `Mutex` and is not
/// reentrant, so acquiring a second fixture inside the first deadlocks.
#[test]
fn dropping_the_guard_restores_the_previous_localizer() {
    let french_before = localizer_is_french();
    {
        let _french = locale_localizer("fr");
        assert!(localizer_is_french(), "the fixture installs French");
    }
    assert_eq!(
        localizer_is_french(),
        french_before,
        "dropping the fixture restores the previous localizer"
    );
}

/// The lock must still be held at the instant the localizer is restored.
///
/// This is the deterministic proof of the field-drop order. A contention test
/// cannot supply one: with the fields declared the wrong way round the window
/// between releasing the lock and restoring the localizer is a few
/// instructions wide, so a waiting thread virtually never lands in it and the
/// test passes on broken code.
///
/// `RestoreProbe::drop` runs immediately before the localizer guard it wraps is
/// dropped, and records whether the lock was held at that moment. Correct order
/// leaves the lock held; the wrong order has already released it.
#[test]
fn the_lock_is_still_held_when_the_restore_begins() {
    *LOCK_HELD_AT_RESTORE
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = None;

    drop(locale_localizer("fr"));

    let observed = *LOCK_HELD_AT_RESTORE
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    assert_eq!(
        observed,
        Some(true),
        "the localizer guard must be dropped while the lock is still held; \
         `false` means the lock was released first, so another test could be \
         admitted before the previous localizer was restored"
    );
}
