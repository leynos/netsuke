//! Tests for the CLI parser's localization rendering.
//!
//! Exercises locale-specific long-help output for the en-US and es-ES
//! locales, asserts that `--config <FILE>` and its Fluent-resolved
//! description are present, and pins the complete rendered help via Insta
//! snapshots to detect regressions in flag naming, ordering, or
//! localization drift.

use super::*;
use crate::cli_localization::build_localizer;
use crate::snapshot_test_support::snapshot_settings;
use insta::assert_snapshot;
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;

/// Verifies localized long-help includes `--config <FILE>` and its
/// Fluent-resolved description, then matches the complete output snapshot.
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

    assert!(
        normalized_help.contains("--config <FILE>"),
        "localized help for {locale} should include the config flag"
    );
    assert!(
        normalized_help.contains(config_help),
        "localized help for {locale} should include the config flag description"
    );
    snapshot_settings("cli").bind(|| {
        assert_snapshot!(snapshot_name, normalized_help);
    });
}

/// Verifies `netsuke help --help` localizes its nested topic descriptions.
#[rstest]
#[case::en_us(
    "en-US",
    [
        "List targets and actions in the selected manifest.",
        "Build targets defined in the manifest",
        "Remove build artefacts via Ninja",
        "Emit the build dependency graph",
        "Generate the Ninja manifest without running Ninja",
    ]
)]
#[case::es_es(
    "es-ES",
    [
        "Enumerar objetivos y acciones en el manifiesto seleccionado.",
        "Compila objetivos definidos en el manifiesto",
        "Elimina artefactos de compilación mediante Ninja",
        "Emite el grafo de dependencias de compilación",
        "Genera el manifiesto Ninja sin ejecutar Ninja",
    ]
)]
fn localized_help_topics_include_localized_descriptions(
    #[case] locale: &str,
    #[case] expected_descriptions: [&str; 5],
) {
    let localizer = build_localizer(Some(locale));
    let mut command = localize_command(Cli::command(), localizer.as_ref());
    let help = command
        .find_subcommand_mut("help")
        .expect("help subcommand should exist");
    let rendered_help = normalize_fluent_isolates(&help.render_long_help().to_string());

    for description in expected_descriptions {
        assert!(
            rendered_help.contains(description),
            "localized help topics for {locale} should contain {description:?}: {rendered_help}"
        );
    }
}
