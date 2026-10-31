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

/// Build a `ValueValidation` error carrying `value` as its only context.
fn value_validation_with(value: ContextValue) -> clap::Error {
    let mut error = clap::Error::raw(ErrorKind::ValueValidation, "rejected");
    error.insert(ContextKind::InvalidValue, value);
    error
}

/// An empty single supplied value must be recognised at the parse boundary.
#[test]
fn empty_single_value_is_recognised() {
    let error = value_validation_with(ContextValue::String(String::new()));
    assert!(
        is_empty_value_validation(&error),
        "an empty supplied value should be recognised"
    );
}

/// A non-empty supplied value still belongs to `ortho_config`'s template path.
#[test]
fn non_empty_single_value_is_not_recognised() {
    let error = value_validation_with(ContextValue::String("bogus".to_owned()));
    assert!(
        !is_empty_value_validation(&error),
        "a non-empty supplied value should keep the catalogue template path"
    );
}

/// A multi-value list whose entries are all empty loses the template argument.
#[test]
fn all_empty_entries_in_multi_value_list_are_recognised() {
    let error = value_validation_with(ContextValue::Strings(vec![String::new(), String::new()]));
    assert!(
        is_empty_value_validation(&error),
        "a multi-value list of empty entries should be recognised"
    );
}

/// A mixed list still joins to a non-empty value, so the template can render.
#[test]
fn mixed_entries_in_multi_value_list_are_not_recognised() {
    let error = value_validation_with(ContextValue::Strings(vec![
        "first".to_owned(),
        String::new(),
    ]));
    assert!(
        !is_empty_value_validation(&error),
        "a mixed multi-value list should keep the catalogue template path"
    );
}

/// A `ValueValidation` error with no supplied value is not this defect.
#[test]
fn missing_value_context_is_not_recognised() {
    let error = clap::Error::raw(ErrorKind::ValueValidation, "rejected");
    assert!(
        !is_empty_value_validation(&error),
        "absent context is not an empty supplied value"
    );
}

/// Other error kinds keep their established localization path.
#[test]
fn other_error_kind_is_not_recognised() {
    let mut error = clap::Error::raw(ErrorKind::InvalidValue, "rejected");
    error.insert(
        ContextKind::InvalidValue,
        ContextValue::String(String::new()),
    );
    assert!(
        !is_empty_value_validation(&error),
        "only value-validation errors take the empty-value boundary path"
    );
}

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

/// Render localized long help for the nested `help targets` topic.
fn render_localized_help_targets_long_help(locale: &str) -> String {
    let localizer: Arc<dyn Localizer> = Arc::from(build_localizer(Some(locale)));
    let mut command = configured_command(Some(&localizer));
    let help = command
        .find_subcommand_mut("help")
        .expect("help subcommand should exist");
    let targets = help
        .find_subcommand_mut("targets")
        .expect("targets topic should exist");
    normalize_fluent_isolates(&targets.render_long_help().to_string())
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

/// Suffix naming the help snapshot for the compiled feature set.
///
/// `netsuke check` exists only with the `lint` feature, so the command list
/// differs by one line. The unsuffixed snapshots are the default feature set,
/// which is what release binaries ship.
const HELP_SNAPSHOT_SUFFIX: &str = if cfg!(feature = "lint") {
    "_with_lint"
} else {
    ""
};

/// Verifies localized `help targets` output documents the opt-in graph flag.
#[rstest]
#[case::en_us("en-US", "Include dependency edges and raw recipes in JSON output.")]
#[case::es_es(
    "es-ES",
    "Incluir aristas de dependencias y recetas sin procesar en la salida JSON."
)]
fn localized_help_targets_documents_edges_flag(#[case] locale: &str, #[case] expected: &str) {
    let rendered_help = render_localized_help_targets_long_help(locale);

    assert!(
        rendered_help.contains("--edges"),
        "help targets for {locale} should include --edges"
    );
    assert!(
        rendered_help.contains(expected),
        "help targets for {locale} should contain {expected:?}: {rendered_help}"
    );
}

/// Accept localized long-help snapshots by reading or writing snapshot files on disk.
#[rstest]
#[case::en_us("en-US", "help_en_us")]
#[case::es_es("es-ES", "help_es_es")]
fn localized_help_snapshot(#[case] locale: &str, #[case] snapshot_name: &str) {
    let rendered_help = render_localized_long_help(locale, None);

    snapshot_settings("cli").bind(|| {
        assert_snapshot!(
            format!("{snapshot_name}{HELP_SNAPSHOT_SUFFIX}"),
            rendered_help
        );
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
