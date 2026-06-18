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

/// RAII bundle holding both the global localiser test lock and the English
/// locale guard for the lifetime of a test.
///
/// Construct via the [`en_localizer`] rstest fixture.  Both guards are
/// released when this value is dropped.
pub struct EnLocalizer {
    _lock: MutexGuard<'static, ()>,
    _guard: LocalizerGuard,
}

/// Rstest fixture that acquires the global localiser test lock and installs
/// the English localiser, returning an [`EnLocalizer`] RAII bundle.
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
    EnLocalizer {
        _lock: localizer_test_lock().expect("localizer test lock poisoned"),
        _guard: set_en_localizer(),
    }
}
