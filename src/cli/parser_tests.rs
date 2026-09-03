//! Tests for the CLI parser's localization rendering.
//!
//! The substring tests exercise locale-specific long-help output as pure
//! queries, whereas the snapshot-acceptance test performs filesystem I/O to
//! verify the complete rendered help through Insta.

use super::*;
use crate::cli_localization::build_localizer;
use crate::snapshot_test_support::snapshot_settings;
use insta::assert_snapshot;
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;

/// Pins the public CLI name independently of the platform executable suffix.
#[test]
fn cli_command_uses_documented_binary_name() {
    assert_eq!(Cli::command().get_bin_name(), Some("netsuke"));
}

/// Render normalized localized long help as a pure query with no filesystem side effects.
fn render_localized_long_help(locale: &str, subcommand: Option<&str>) -> String {
    let localizer: Arc<dyn Localizer> = Arc::from(build_localizer(Some(locale)));
    let mut command = configured_command(Some(&localizer));
    let help_command = match subcommand {
        Some(name) => {
            let Some(requested_subcommand) = command.find_subcommand_mut(name) else {
                panic!("requested subcommand should exist");
            };

            requested_subcommand
        }
        None => &mut command,
    };

    let rendered = normalize_fluent_isolates(&help_command.render_long_help().to_string());
    let has_trailing_newline = rendered.ends_with('\n');
    let mut normalized = rendered
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    if has_trailing_newline {
        normalized.push('\n');
    }
    normalized
}

/// Verifies localized long help includes `--config <FILE>` and its Fluent-resolved description.
#[rstest]
#[case::en_us(
    "en-US",
    "Path to a configuration file, bypassing automatic discovery."
)]
#[case::es_es(
    "es-ES",
    "Ruta a un archivo de configuración, omitiendo la detección automática."
)]
fn localized_help_includes_config_flag(#[case] locale: &str, #[case] config_help: &str) {
    let rendered_help = render_localized_long_help(locale, None);

    assert!(
        rendered_help.contains("--config <FILE>"),
        "localized help for {locale} should include the config flag"
    );
    assert!(
        rendered_help.contains(config_help),
        "localized help for {locale} should include the config flag description"
    );
}

/// Accept localized long-help snapshots by reading or writing snapshot files on disk.
#[rstest]
#[case::en_us("en-US", "help_en_us")]
#[case::es_es("es-ES", "help_es_es")]
fn localized_help_snapshot(#[case] locale: &str, #[case] snapshot_name: &str) {
    let rendered_help = render_localized_long_help(locale, None);

    snapshot_settings("cli").bind(|| {
        assert_snapshot!(snapshot_name, rendered_help);
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
    let rendered_help = render_localized_long_help(locale, Some("help"));

    for description in expected_descriptions {
        assert!(
            rendered_help.contains(description),
            "localized help topics for {locale} should contain {description:?}: {rendered_help}"
        );
    }
}
