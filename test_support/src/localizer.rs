//! Test helpers for localizer configuration.

use netsuke::cli_localization;
use netsuke::localization;
pub use netsuke::localization::LocalizerGuard;
use rstest::fixture;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

/// Mutex used to serialize process-wide localizer mutations in tests.
pub static LOCALIZER_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Acquire the global localizer test lock.
pub fn localizer_test_lock() -> Result<MutexGuard<'static, ()>, PoisonError<MutexGuard<'static, ()>>>
{
    LOCALIZER_TEST_LOCK.get_or_init(|| Mutex::new(())).lock()
}

/// Install the English localizer for tests.
pub fn set_en_localizer() -> LocalizerGuard {
    let localizer = cli_localization::build_localizer(Some("en-US"));
    localization::set_localizer_for_tests(Arc::from(localizer))
}

/// RAII bundle holding both the global localizer test lock and the English
/// locale guard for the lifetime of a test.
///
/// Construct via the [`en_localizer`] rstest fixture.  Both guards are
/// released when this value is dropped.
pub struct EnLocalizer {
    _lock: MutexGuard<'static, ()>,
    _guard: LocalizerGuard,
}

/// Rstest fixture that acquires the global localizer test lock and installs
/// the English localizer, returning an [`EnLocalizer`] RAII bundle.
///
/// Bind the returned value immediately in each test body:
///
/// ```rust,ignore
/// #[rstest]
/// fn my_test(en_localizer: EnLocalizer) {
///     let _en_localizer = en_localizer;
///     // … assertions …
/// }
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
        _lock: lock,
        _guard: set_en_localizer(),
    }
}

#[cfg(test)]
mod tests {
    //! Coverage for the poisoned-lock recovery in the [`en_localizer`] fixture.

    use super::{LOCALIZER_TEST_LOCK, en_localizer, localizer_test_lock};
    use std::{panic, thread};

    /// Poison the lock the way a panicking test would: hold the guard across a
    /// panic on another thread.
    ///
    /// The default panic hook is suppressed for the duration so the deliberate
    /// panic does not print a misleading backtrace during an otherwise passing
    /// run.
    ///
    /// The whole `Result` is bound rather than unwrapped: the guard lives
    /// inside either variant, so holding it across the panic poisons the mutex
    /// without this helper — which Whitaker does not recognise as test code —
    /// needing an `expect`.
    fn poison_localizer_test_lock() {
        let hook = panic::take_hook();
        panic::set_hook(Box::new(|_| {}));
        let poisoner = thread::spawn(|| {
            let _guard = localizer_test_lock();
            panic!("deliberately poisoning LOCALIZER_TEST_LOCK");
        });
        let outcome = poisoner.join();
        panic::set_hook(hook);
        assert!(
            outcome.is_err(),
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
    }
}
