//! Tests for Netsuke's JSON diagnostics schema.

use super::{render_diagnostic_json, render_error_json};
use crate::localization::{self, keys};
use crate::manifest;
use crate::runner::RunnerError;
use anyhow::{Context, Result, ensure};
use insta::{Settings, assert_snapshot};
use rstest::rstest;
use serde_json::{Map, Value};
use std::path::PathBuf;
use test_support::{localizer_test_lock, set_en_localizer};

fn parse_json_value(document: &str) -> Result<Value> {
    serde_json::from_str(document).context("parse diagnostics JSON")
}

fn snapshot_settings() -> Settings {
    let mut settings = Settings::new();
    settings.set_snapshot_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/snapshots/diagnostic_json"
    ));
    settings
}

fn first_diagnostic(value: &Value) -> Result<&Map<String, Value>> {
    value
        .get("diagnostics")
        .and_then(Value::as_array)
        .and_then(|diagnostics| diagnostics.first())
        .and_then(Value::as_object)
        .context("diagnostic entry should be an object")
}

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
