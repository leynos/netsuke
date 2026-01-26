//! Tests covering localization helpers and fallback behaviour.

use std::sync::Arc;

use anyhow::{Result, ensure};
use rstest::rstest;
use test_support::localizer_test_lock;

use netsuke::cli_localization;
use netsuke::localization::{self, keys};

fn which_message(command: &str) -> String {
    localization::message(keys::STDLIB_WHICH_NOT_FOUND)
        .with_arg("command", command)
        .with_arg("count", 0)
        .with_arg("preview", "<none>")
        .to_string()
}

#[rstest]
#[case("es-ES", "no encontrado")]
#[case("fr-FR", "not found")]
fn localisation_resolves_expected_message(
    #[case] locale: &str,
    #[case] expected_substring: &str,
) -> Result<()> {
    let _lock = localizer_test_lock();
    let localizer = cli_localization::build_localizer(Some(locale));
    let _guard = localization::set_localizer_for_tests(Arc::from(localizer));

    let message = which_message("tool");
    ensure!(
        message.contains(expected_substring),
        "expected message to contain {expected_substring:?} for locale {locale}, got: {message}"
    );
    Ok(())
}
