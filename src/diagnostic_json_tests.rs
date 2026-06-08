//! Tests for Netsuke's JSON diagnostics schema.

use super::{render_diagnostic_json, render_error_json};
use crate::ir::IrGenError;
use crate::localization::{self, keys};
use crate::manifest;
use crate::runner::RunnerError;
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use insta::{Settings, assert_snapshot};
use rstest::{fixture, rstest};
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::sync::MutexGuard;
use test_support::{LocalizerGuard, localizer_test_lock, set_en_localizer};

/// RAII bundle holding both the global localizer test lock and the locale guard for a test.
struct EnLocalizer {
    _lock: MutexGuard<'static, ()>,
    _guard: LocalizerGuard,
}

/// Rstest fixture that acquires the localizer test lock and installs the English localiser.
#[fixture]
fn en_localizer() -> EnLocalizer {
    EnLocalizer {
        _lock: localizer_test_lock().expect("localizer test lock poisoned"),
        _guard: set_en_localizer(),
    }
}

/// Parses a JSON string into a [`serde_json::Value`].
fn parse_json_value(document: &str) -> Result<Value> {
    serde_json::from_str(document).context("parse diagnostics JSON")
}

/// Builds insta [`Settings`] pointing at the `src/snapshots/diagnostic_json` directory.
fn snapshot_settings() -> Settings {
    let mut settings = Settings::new();
    settings.set_snapshot_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/snapshots/diagnostic_json"
    ));
    settings
}

/// Extracts the first diagnostic object from the top-level `diagnostics` array.
fn first_diagnostic(value: &Value) -> Result<&Map<String, Value>> {
    value
        .get("diagnostics")
        .and_then(Value::as_array)
        .and_then(|diagnostics| diagnostics.first())
        .and_then(Value::as_object)
        .context("diagnostic entry should be an object")
}

/// Constructs a deterministic [`RunnerError::ManifestNotFound`] fixture.
fn manifest_not_found_error() -> RunnerError {
    RunnerError::ManifestNotFound {
        manifest_name: String::from("Netsukefile"),
        directory: String::from("the current directory"),
        path: PathBuf::from("/workspace/Netsukefile"),
        message: localization::message(keys::RUNNER_MANIFEST_NOT_FOUND)
            .with_arg("manifest_name", "Netsukefile")
            .with_arg("directory", "the current directory"),
        help: localization::message(keys::RUNNER_MANIFEST_NOT_FOUND_HELP),
    }
}

/// Constructs an [`IrGenError::CircularDependency`] for the given cycle path.
///
/// `cycle_nodes` must contain at least two elements, and the first and last
/// elements must be identical (i.e. it must form a valid closed cycle).
fn circular_dependency_error_for(cycle_nodes: Vec<&str>) -> IrGenError {
    let cycle: Vec<Utf8PathBuf> = cycle_nodes.into_iter().map(Utf8PathBuf::from).collect();
    let message =
        localization::message(keys::IR_CIRCULAR_DEPENDENCY).with_arg("cycle", format!("{cycle:?}"));
    IrGenError::CircularDependency {
        cycle,
        missing_dependencies: Vec::new(),
        message,
    }
}

/// Constructs the canonical three-node circular-dependency fixture used by
/// snapshot tests.
fn circular_dependency_error() -> IrGenError {
    circular_dependency_error_for(vec!["a", "b", "a"])
}

#[rstest]
fn render_plain_error_json_records_cause_chain() -> Result<()> {
    let error = anyhow::anyhow!("top level failure").context("outer context");
    let document = render_error_json(error.as_ref())?;
    let value = parse_json_value(&document)?;
    let diagnostic = first_diagnostic(&value)?;
    let schema_version = value
        .get("schema_version")
        .and_then(Value::as_i64)
        .context("schema version should be present")?;
    let generator_name = value
        .get("generator")
        .and_then(Value::as_object)
        .and_then(|generator| generator.get("name"))
        .and_then(Value::as_str)
        .context("generator name should be present")?;
    let message = diagnostic
        .get("message")
        .and_then(Value::as_str)
        .context("message should be present")?;
    let causes = diagnostic
        .get("causes")
        .context("causes should be present")?;
    let labels = diagnostic
        .get("labels")
        .context("labels should be present")?;

    ensure!(schema_version == 1, "schema version should be stable",);
    ensure!(
        generator_name == "netsuke",
        "generator name should be present",
    );
    ensure!(
        message == "outer context",
        "plain errors should use the top-level message"
    );
    ensure!(
        causes == &Value::from(vec![String::from("top level failure")]),
        "plain errors should record the error cause chain",
    );
    ensure!(
        labels == &Value::Array(Vec::new()),
        "plain errors should not fabricate labels",
    );
    Ok(())
}

#[rstest]
fn render_runner_diagnostic_json_records_help_without_spans() -> Result<()> {
    let _lock = localizer_test_lock().expect("localizer test lock poisoned");
    let _guard = set_en_localizer();
    let document = render_diagnostic_json(&manifest_not_found_error())?;
    let value = parse_json_value(&document)?;
    let diagnostic = first_diagnostic(&value)?;
    let code = diagnostic
        .get("code")
        .and_then(Value::as_str)
        .context("diagnostic code should be present")?;
    let help = diagnostic
        .get("help")
        .and_then(Value::as_str)
        .context("diagnostic help should be present")?;
    let source = diagnostic
        .get("source")
        .context("source should be present")?;
    let labels = diagnostic
        .get("labels")
        .context("labels should be present")?;

    ensure!(
        code == "netsuke::runner::manifest_not_found",
        "runner diagnostic code should be stable",
    );
    ensure!(
        help == "Ensure the manifest exists or pass `--file` with the correct path.",
        "runner diagnostics should include help text",
    );
    ensure!(
        source.is_null(),
        "manifest-not-found should not claim a source file span"
    );
    ensure!(
        labels == &Value::Array(Vec::new()),
        "manifest-not-found should not include labels",
    );
    Ok(())
}

/// Verifies structural invariants of `IrGenError::CircularDependency` across
/// cycle lengths without relying on snapshot output.
#[rstest]
#[case(vec!["x", "x"], "minimal two-node cycle")]
#[case(vec!["a", "b", "a"], "three-node cycle")]
#[case(vec!["a", "b", "c", "a"], "four-node cycle")]
#[case(vec!["p", "q", "r", "s", "p"], "five-node cycle")]
fn circular_dependency_structural_invariants(
    en_localizer: EnLocalizer,
    #[case] cycle_nodes: Vec<&str>,
    #[case] description: &str,
) {
    let _en_localizer = en_localizer;
    let error = circular_dependency_error_for(cycle_nodes.clone());

    if let IrGenError::CircularDependency {
        cycle,
        missing_dependencies,
        ..
    } = &error
    {
        assert_eq!(
            cycle.len(),
            cycle_nodes.len(),
            "{description}: cycle length must match the input"
        );
        assert_eq!(
            cycle.first(),
            cycle.last(),
            "{description}: cycle must be closed: first and last nodes must be equal"
        );
        assert!(
            missing_dependencies.is_empty(),
            "{description}: fixture must not report missing dependencies"
        );
        let rendered = error.to_string();
        for node in &cycle_nodes {
            assert!(
                rendered.contains(node),
                "{description}: Display output must contain node {node:?}: got {rendered:?}"
            );
        }
    } else {
        panic!("expected CircularDependency variant");
    }
}

#[rstest]
fn render_circular_dependency_display_matches_snapshot(en_localizer: EnLocalizer) {
    let _en_localizer = en_localizer;
    let rendered = circular_dependency_error().to_string();

    snapshot_settings().bind(|| {
        assert_snapshot!("circular_dependency_display", rendered);
    });
}

#[rstest]
fn render_circular_dependency_json_matches_snapshot(en_localizer: EnLocalizer) -> Result<()> {
    let _en_localizer = en_localizer;
    let error = anyhow::Error::new(circular_dependency_error())
        .context(localization::message(keys::RUNNER_CONTEXT_BUILD_GRAPH));
    let document = render_error_json(error.as_ref())?;
    let value = parse_json_value(&document)?;
    let rendered =
        serde_json::to_string_pretty(&value).context("render diagnostic JSON snapshot value")?;

    snapshot_settings().bind(|| {
        assert_snapshot!("circular_dependency_json", rendered);
    });
    Ok(())
}

/// Verifies that `render_error_json` for a `CircularDependency` error produces
/// a well-formed diagnostics document with a non-empty cause chain.
#[rstest]
fn render_circular_dependency_json_has_expected_shape(en_localizer: EnLocalizer) -> Result<()> {
    let _en_localizer = en_localizer;
    let error = anyhow::Error::new(circular_dependency_error())
        .context(localization::message(keys::RUNNER_CONTEXT_BUILD_GRAPH));
    let document = render_error_json(error.as_ref())?;
    let value = parse_json_value(&document)?;
    let diagnostic = first_diagnostic(&value)?;

    let causes = diagnostic
        .get("causes")
        .and_then(Value::as_array)
        .context("circular dependency JSON must include a causes array")?;
    let message = diagnostic
        .get("message")
        .and_then(Value::as_str)
        .context("circular dependency JSON must include a message string")?;

    ensure!(
        !causes.is_empty(),
        "circular dependency must have at least one cause"
    );
    ensure!(
        !message.is_empty(),
        "circular dependency message must not be empty"
    );
    Ok(())
}

#[rstest]
fn render_manifest_parse_diagnostic_matches_snapshot() -> Result<()> {
    let _lock = localizer_test_lock().expect("localizer test lock poisoned");
    let _guard = set_en_localizer();
    let err = manifest::from_str("targets:\n\t- name: test\n")
        .expect_err("invalid YAML should fail to parse");
    let manifest_err = err
        .downcast_ref::<manifest::ManifestError>()
        .context("expected ManifestError")?;
    let document = render_diagnostic_json(manifest_err)?;
    let value = parse_json_value(&document)?;
    let rendered =
        serde_json::to_string_pretty(&value).context("render diagnostic JSON snapshot value")?;

    snapshot_settings().bind(|| {
        assert_snapshot!("manifest_parse_error", rendered);
    });
    Ok(())
}
