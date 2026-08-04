//! Test helpers for localizer configuration.

use netsuke::cli_localization;
use netsuke::localization;
pub use netsuke::localization::LocalizerGuard;
use rstest::fixture;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

/// Mutex used to serialize process-wide localizer mutations in tests.
pub static LOCALIZER_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Acquire the global localizer test lock.
///
/// # Errors
///
/// Returns an error if another test poisoned the localizer mutex.
pub fn localizer_test_lock() -> Result<MutexGuard<'static, ()>, PoisonError<MutexGuard<'static, ()>>>
{
    LOCALIZER_TEST_LOCK.get_or_init(|| Mutex::new(())).lock()
}

/// Install the English localizer for tests.
#[must_use]
pub fn set_en_localizer() -> LocalizerGuard {
    let localizer = cli_localization::build_localizer(Some("en-US"));
    localization::set_localizer_for_tests(Arc::from(localizer))
}

/// Records whether the test lock was still held when the localizer guard
/// began to drop.
///
/// This is the seam that makes the drop-order invariant observable. The
/// ordering it protects has no behavioural signature under normal scheduling —
/// a waiting thread almost never lands inside the window — so a contention
/// test alone cannot distinguish correct from incorrect field order.
#[cfg(test)]
pub(crate) static LOCK_HELD_AT_RESTORE: Mutex<Option<bool>> = Mutex::new(None);

/// Wraps the localizer guard so the moment it drops can be observed.
///
/// Transparent in production: the wrapper exists so that a test can ask
/// whether the lock was still held at the instant the restore began. The
/// wrapper's `Drop` body runs before its field is dropped, which is exactly
/// that instant.
pub struct RestoreProbe {
    /// Held for its `Drop`, which restores the previous localizer. Never read:
    /// the value has no API, only an effect at end of scope. The leading
    /// underscore is what exempts it from `dead_code`, rather than an
    /// expectation that would outlive any work it could be linked to.
    _guard: LocalizerGuard,
}

impl RestoreProbe {
    fn new(guard: LocalizerGuard) -> Self {
        Self { _guard: guard }
    }
}

impl Drop for RestoreProbe {
    fn drop(&mut self) {
        #[cfg(test)]
        {
            // `try_lock` from the thread already holding it returns
            // `WouldBlock`: `std::sync::Mutex` is not reentrant. So "blocked"
            // means the guard bundle still holds the lock, and "acquired"
            // means the lock was released before the restore — the fault.
            // Only `WouldBlock` is evidence of that: a poisoned result means
            // the lock was acquirable, so counting it as held would mask the
            // very release-before-restore fault this probe exists to catch.
            let held = LOCALIZER_TEST_LOCK.get().is_some_and(|lock| {
                matches!(lock.try_lock(), Err(std::sync::TryLockError::WouldBlock))
            });
            let mut slot = LOCK_HELD_AT_RESTORE
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            *slot = Some(held);
        }
    }
}
/// RAII bundle holding both the global localizer test lock and the English
/// locale guard for the lifetime of a test.
///
/// Construct via the [`en_localizer`] rstest fixture.  Both guards are
/// released when this value is dropped.
///
/// Field order is load-bearing: struct fields drop in declaration order, so
/// `_guard` must precede `_lock`. The localizer restoration in
/// `LocalizerGuard::drop` writes the global localizer, which is exactly what
/// the test lock serializes; releasing the lock first would let a waiting
/// thread install its own override and capture this test's override as its
/// "previous", so that thread would later restore the wrong value.
pub struct EnLocalizer {
    // Field order is the invariant: Rust drops fields in declaration order, so
    // the localizer guard must come first. It restores the process-global
    // localizer, and doing that after the mutex was released would let another
    // test install its own localizer into the window and have it overwritten.
    _guard: RestoreProbe,
    _lock: MutexGuard<'static, ()>,
}

/// Rstest fixture that acquires the global localizer test lock and installs
/// the English localizer, returning an [`EnLocalizer`] RAII bundle.
///
/// Bind the returned value immediately in each test body, since dropping it
/// straight away would release the localizer before assertions run.
///
/// # Examples
///
/// `#[rstest]` expands a test taking `en_localizer: EnLocalizer` into a
/// zero-argument `#[test]` function that only `cargo test` can invoke, so it
/// is shown here for the usage pattern and defined but not called directly.
/// The shared `assert_localized` helper carries the actual assertion, and is
/// called both from the illustrated test and directly below so this example
/// still exercises real, meaningful output when run as a doctest.
///
/// ```rust
/// use netsuke::localization::{keys::CLI_ABOUT, message};
/// use rstest::rstest;
/// use test_support::localizer::{en_localizer, EnLocalizer};
///
/// fn assert_localized(en_localizer: EnLocalizer) {
///     let _en_localizer = en_localizer;
///
///     let resolved = message(CLI_ABOUT).to_string();
///
///     // A resolved message differs from the raw key; a match would mean the
///     // Fluent catalogue failed to load and lookup fell back to the key.
///     assert_ne!(resolved, CLI_ABOUT);
/// }
///
/// #[rstest]
/// fn resolves_localized_cli_about(en_localizer: EnLocalizer) {
///     assert_localized(en_localizer);
/// }
///
/// assert_localized(en_localizer());
/// ```
#[fixture]
pub fn en_localizer() -> EnLocalizer {
    // A poisoned lock means an earlier test panicked while holding it. The lock
    // guards nothing but the ordering of localizer installation, and
    // `set_en_localizer` below re-establishes the global state unconditionally,
    // so recovering the guard is safe. Panicking here would instead fail every
    // subsequent test that takes this fixture. `crate::env_lock` recovers from
    // poisoning the same way.
    let lock = localizer_test_lock().unwrap_or_else(PoisonError::into_inner);
    EnLocalizer {
        _guard: RestoreProbe::new(set_en_localizer()),
        _lock: lock,
    }
}

/// RAII bundle holding the localizer test lock and an arbitrary locale.
///
/// [`EnLocalizer`] covers the common case of pinning English. Catalogue sweeps
/// need the same pairing for each locale in turn, which is what this provides.
///
/// Obtained from [`locale_localizer`]. Dropping it restores the localizer that
/// was installed beforehand and *then* releases the shared test lock, in that
/// order — so no other test can be admitted into the window between the two and
/// have its own localizer overwritten.
///
/// The ordering is the field declaration order, since Rust drops fields in the
/// order they are declared; see [`EnLocalizer`] for the same arrangement.
pub struct LocaleLocalizer {
    _guard: RestoreProbe,
    _lock: MutexGuard<'static, ()>,
}

/// Acquire the localizer test lock and install the localizer for `locale`.
///
/// Infallible, like [`en_localizer`] and for the same reason: a poisoned lock
/// is recovered from rather than reported, and building a localizer cannot
/// fail — an unsupported tag resolves to the English source catalogue. There
/// is no error for a caller to decide about, so returning a `Result` would
/// only oblige every call site to unwrap one that is always `Ok`.
///
/// Dropping the returned guard restores the previously installed localizer and
/// releases the shared test lock, so locale-specific tests can run in sequence
/// without leaking state into one another.
///
/// # Examples
///
/// ```
/// use netsuke::localization::{self, keys};
/// use test_support::localizer::locale_localizer;
///
/// let guard = locale_localizer("fr");
/// let rendered = localization::message(keys::CLI_ABOUT).to_string();
/// assert!(rendered.contains("Netsuke"));
/// drop(guard); // the previous localizer is restored here
/// ```
#[must_use]
pub fn locale_localizer(locale: &str) -> LocaleLocalizer {
    // Lock first, then install: the guard returned by
    // `set_localizer_for_tests` captures the localizer to restore, so it must
    // be created under the lock.
    // Poisoning is recovered from, as `en_localizer` does and for the same
    // reason: the lock orders localizer installation and nothing more, and the
    // installation below re-establishes the global state unconditionally.
    // Propagating instead would make one panicking test fail every later test
    // that takes this lock, long after the original failure.
    let lock = localizer_test_lock().unwrap_or_else(PoisonError::into_inner);
    let localizer = cli_localization::build_localizer(Some(locale));
    LocaleLocalizer {
        _guard: RestoreProbe::new(localization::set_localizer_for_tests(Arc::from(localizer))),
        _lock: lock,
    }
}

#[cfg(test)]
mod tests {
    //! Coverage for the poisoned-lock recovery in the [`en_localizer`] fixture.

    use super::{LOCALIZER_TEST_LOCK, en_localizer, localizer_test_lock};
    use std::thread;

    /// Poison the lock the way a panicking test would: hold the guard across a
    /// panic on another thread.
    ///
    /// The panic hook is deliberately left alone. `panic::set_hook` is
    /// process-wide, so swapping it out for the duration would suppress — or,
    /// if two threads raced on take/restore, permanently replace — the hook any
    /// concurrently panicking test relies on to report itself. The stderr noise
    /// from one deliberate panic is the cheaper cost.
    ///
    /// The whole `Result` is bound rather than unwrapped: the guard lives
    /// inside either variant, so holding it across the panic poisons the mutex
    /// without this helper — which Whitaker does not recognise as test code —
    /// needing an `expect`.
    fn poison_localizer_test_lock() {
        let poisoner = thread::spawn(|| {
            let _guard = localizer_test_lock();
            panic!("deliberately poisoning LOCALIZER_TEST_LOCK");
        });
        assert!(
            poisoner.join().is_err(),
            "the poisoning thread should have panicked"
        );
    }

    #[test]
    fn en_localizer_recovers_from_a_poisoned_lock() {
        poison_localizer_test_lock();
        assert!(
            LOCALIZER_TEST_LOCK
                .get()
                .is_some_and(std::sync::Mutex::is_poisoned),
            "the lock should be poisoned before exercising recovery"
        );

        // The fixture must recover the guard rather than propagate the poison;
        // panicking here would fail every later test that takes the fixture.
        let bundle = en_localizer();
        drop(bundle);

        // Recovery does not clear the poison flag, so a second call must also
        // succeed rather than depending on the first having reset it.
        drop(en_localizer());

        // Leave the static as it was found. The flag is sticky and the lock is
        // shared with every other test that takes this fixture, so a deliberate
        // poisoning must not outlive the test that caused it.
        if let Some(lock) = LOCALIZER_TEST_LOCK.get() {
            lock.clear_poison();
        }
        assert!(
            LOCALIZER_TEST_LOCK
                .get()
                .is_some_and(|lock| !lock.is_poisoned()),
            "the poison flag should be cleared before leaving the test"
        );
    }
}

#[cfg(test)]
#[path = "localizer_tests.rs"]
mod drop_order_tests;
