//! Tests covering localisation helpers and fallback behaviour.

use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Result, ensure};
use rstest::rstest;

use netsuke::cli_localization;
use netsuke::localization::{self, keys};

static LOCALIZER_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn localizer_test_lock() -> std::sync::MutexGuard<'static, ()> {
    LOCALIZER_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| panic!("localizer test lock poisoned: {err}"))
}

fn which_message(command: &str) -> String {
    localization::message(keys::STDLIB_WHICH_NOT_FOUND)
        .with_arg("command", command)
        .with_arg("count", 0)
        .with_arg("preview", "<none>")
        .to_string()
}

#[rstest]
fn localisation_uses_spanish_messages() -> Result<()> {
    let _lock = localizer_test_lock();
    let localizer = cli_localization::build_localizer(Some("es-ES"));
    let _guard = localization::set_localizer_for_tests(Arc::from(localizer));

    let message = which_message("tool");
    ensure!(
        message.contains("no encontrado"),
        "expected Spanish translation, got: {message}"
    );
    Ok(())
}

#[rstest]
fn localisation_falls_back_to_english_for_unknown_locale() -> Result<()> {
    let _lock = localizer_test_lock();
    let localizer = cli_localization::build_localizer(Some("fr-FR"));
    let _guard = localization::set_localizer_for_tests(Arc::from(localizer));

    let message = which_message("tool");
    ensure!(
        message.contains("not found"),
        "expected English fallback, got: {message}"
    );
    Ok(())
}
