//! Test helpers for localizer configuration.

use netsuke::cli_localization;
use netsuke::localization::{self, LocalizerGuard};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

/// Mutex used to serialize process-wide localizer mutations in tests.
pub static LOCALIZER_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Acquire the global localizer test lock.
pub fn localizer_test_lock() -> MutexGuard<'static, ()> {
    LOCALIZER_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("localizer test lock poisoned")
}

/// Install the English localizer for tests.
pub fn set_en_localizer() -> LocalizerGuard {
    let localizer = cli_localization::build_localizer(Some("en-US"));
    localization::set_localizer_for_tests(Arc::from(localizer))
}
