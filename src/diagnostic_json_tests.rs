//! Snapshot tests for Netsuke's JSON diagnostics schema.
//!
//! The diagnostic snapshot filter is one anchored regular expression with no
//! data-dependent branching. A fixed example that places unrelated `version`
//! fields on both sides of the matching generator block therefore covers every
//! filter path; property generation would not exercise a distinct behaviour.
//!
//! Assertions that parse a rendered document and inspect its fields live in the
//! `shape_tests` submodule. They are split out because insta derives a stored
//! snapshot's file name from the declaring module path, so the tests below must
//! declare their snapshots here.

use super::{render_diagnostic_json, render_error_json};
use crate::ir::IrGenError;
use crate::localization::{self, keys};
use crate::manifest;
use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use insta::assert_snapshot;
use rstest::rstest;
use serde_json::Value;
use test_support::{EnLocalizer, en_localizer};

use crate::snapshot_test_support::diagnostic_json_snapshot_settings as snapshot_settings;

#[path = "diagnostic_json_shape_tests.rs"]
mod shape_tests;

/// Parses a JSON string into a [`serde_json::Value`].
fn parse_json_value(document: &str) -> Result<Value> {
    serde_json::from_str(document).context("parse diagnostics JSON")
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

/// The redaction filter must touch only the generator's version: a `version`
/// field elsewhere in a rendered document has to stay visible in snapshot
/// diffs, so drift in other versioned content remains detectable.
#[rstest]
fn snapshot_filter_preserves_versions_outside_the_generator_block() {
    let rendered = concat!(
        "{\n",
        "  \"schema\": {\n",
        "    \"version\": \"3.4.5\"\n",
        "  },\n",
        "  \"generator\": {\n",
        "    \"name\": \"netsuke\",\n",
        "    \"version\": \"9.9.9\"\n",
        "  },\n",
        "  \"tool\": {\n",
        "    \"name\": \"netsuke\",\n",
        "    \"version\": \"1.2.3\"\n",
        "  }\n",
        "}"
    );

    snapshot_settings().bind(|| {
        assert_snapshot!(rendered, @r#"
        {
          "schema": {
            "version": "3.4.5"
          },
          "generator": {
            "name": "netsuke",
            "version": "[version]"
          },
          "tool": {
            "name": "netsuke",
            "version": "1.2.3"
          }
        }
        "#);
    });
}

/// Asserts that the `Display` output of `IrGenError::CircularDependency`
/// matches the stored insta snapshot, preserving the user-facing format.
#[rstest]
fn render_circular_dependency_display_matches_snapshot(en_localizer: EnLocalizer) {
    let _en_localizer = en_localizer;
    let rendered = circular_dependency_error().to_string();

    snapshot_settings().bind(|| {
        assert_snapshot!("circular_dependency_display", rendered);
    });
}

/// Asserts that the JSON diagnostic output for `IrGenError::CircularDependency`
/// wrapped in a build-graph context matches the stored insta snapshot.
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

/// Asserts that the JSON diagnostic output for a YAML parse error in the
/// manifest matches the stored insta snapshot.
#[rstest]
fn render_manifest_parse_diagnostic_matches_snapshot(en_localizer: EnLocalizer) -> Result<()> {
    let _en_localizer = en_localizer;
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
