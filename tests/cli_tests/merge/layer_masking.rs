//! Characterization of scalar masking across configuration layers.
//!
//! The merge composes each layer's JSON object and deserializes the selected
//! value once. These tests pin that a higher-priority valid value masks a lower
//! invalid value, and that an invalid winning value still rejects the merge.
//! The diagnostic resolver relies on the first result when it short-circuits on
//! an explicit `--json` flag.

use anyhow::{Result, ensure};
use netsuke::cli::{CliConfig, ProgressPolicy};
use ortho_config::MergeComposer;
use rstest::{fixture, rstest};
use serde_json::{Value, json};

/// Reuse the parent module's sanitized default `CliConfig` layer.
#[fixture]
fn defaults() -> Result<Value> {
    super::default_cli_json()
}

/// Merge defaults, one file layer, and one environment layer.
///
/// The command-line layer is deliberately absent: these cases characterize the
/// lower layers so the diagnostic resolver's `--json` short circuit can be
/// judged against them.
fn merge_file_and_environment(
    defaults: Value,
    file_layer: Value,
    environment_layer: Value,
) -> Result<CliConfig> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(defaults);
    composer.push_file(file_layer, None);
    composer.push_environment(environment_layer);
    CliConfig::merge_from_layers(composer.layers()).map_err(anyhow::Error::from)
}

/// A valid higher layer masks invalid Boolean and policy values below it.
#[rstest]
fn higher_valid_value_masks_lower_invalid_value(defaults: Result<Value>) -> Result<()> {
    let merged = merge_file_and_environment(
        defaults?,
        json!({ "json": "yes", "progress": "bogus" }),
        json!({ "json": true, "progress": "always" }),
    )?;

    ensure!(
        merged.json,
        "the environment layer's valid json value should win over the invalid file value",
    );
    ensure!(
        merged.progress == ProgressPolicy::Always,
        "the environment layer's valid progress value should win over the invalid file value, got {:?}",
        merged.progress,
    );
    Ok(())
}

/// Without a higher layer, an invalid low-priority value rejects the merge.
#[rstest]
fn lower_invalid_value_without_override_is_rejected(defaults: Result<Value>) -> Result<()> {
    let error = merge_file_and_environment(defaults?, json!({ "json": "yes" }), json!({}))
        .expect_err("an invalid Boolean with no higher layer must be rejected");

    ensure!(
        error.to_string().contains("invalid type"),
        "expected an invalid-type rejection, got: {error}",
    );
    Ok(())
}

/// An invalid winning value rejects the merge even when a valid lower layer
/// exists.
#[rstest]
fn higher_invalid_value_rejects_valid_lower_value(defaults: Result<Value>) -> Result<()> {
    let error =
        merge_file_and_environment(defaults?, json!({ "json": true }), json!({ "json": "yes" }))
            .expect_err("an invalid winning Boolean must reject the merge");

    ensure!(
        error.to_string().contains("invalid type"),
        "expected an invalid-type rejection, got: {error}",
    );
    Ok(())
}
