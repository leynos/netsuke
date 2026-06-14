//! Parser rendering tests for localized CLI long-help output.
//!
//! These tests exercise the `parser.rs` localization path for `en-US` and
//! `es-ES`, assert that `--config <FILE>` and its Fluent-resolved description
//! are present, and pin the complete rendered help with Insta snapshots.

use super::*;
use crate::cli_localization::build_localizer;
use crate::snapshot_test_support::snapshot_settings;
use insta::assert_snapshot;
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;

#[rstest]
#[case::en_us(
    "en-US",
    "help_en_us",
    "Path to a configuration file, bypassing automatic discovery."
)]
#[case::es_es(
    "es-ES",
    "help_es_es",
    "Ruta a un archivo de configuración, omitiendo la detección automática."
)]
fn localized_help_snapshots_include_config_flag(
    #[case] locale: &str,
    #[case] snapshot_name: &str,
    #[case] config_help: &str,
) {
    let localizer = build_localizer(Some(locale));
    let mut command = localize_command(Cli::command(), localizer.as_ref());
    let rendered_help = command.render_long_help().to_string();
    let normalized_help = normalize_fluent_isolates(&rendered_help);
    let missing_config_flag = format!("localized help for {locale} should include the config flag");
    let missing_config_description =
        format!("localized help for {locale} should include the config flag description");

    assert!(
        normalized_help.contains("--config <FILE>"),
        "{missing_config_flag}"
    );
    assert!(
        normalized_help.contains(config_help),
        "{missing_config_description}"
    );
    snapshot_settings("cli").bind(|| {
        assert_snapshot!(snapshot_name, normalized_help);
    });
}
