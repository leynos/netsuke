//! Snapshot the graph export schema and its raw recipe boundary.

use super::super::{build_catalogue, graph_json, render_json};
use crate::ast::NetsukeManifest;
use crate::snapshot_test_support::help_targets_json_snapshot_settings;
use anyhow::{Result, ensure};
use insta::assert_snapshot;

/// Parse a manifest fixture that exercises the closed export schema.
fn graph_fixture_manifest() -> Result<NetsukeManifest> {
    let yaml = r#"netsuke_version: "1.0.0"
rules:
  - name: compile
    command: 'cc {{ ins }} -o {{ outs }}'
actions:
  - name: [check, verify]
    description: Check the source tree
    command: 'command_available("cargo") {{ ins }} {{ outs }}'
  - name: prepare
    command:
      - 'echo {{ ins }}'
      - 'echo {{ outs }}'
targets:
  - name: scripted
    description: Run the script gate
    script: |
      echo {{ ins }} {{ outs }}
    sources: input.c
    deps: [check]
    order_only_deps: [ready]
    dependency_order: serial
    always: true
  - name: built
    rule: compile
  - name: aggregate
    deps: [built]
  - name: choice
    command: 'echo first'
  - name: choice
    command: 'echo second'
defaults: [check, built, built]
"#;
    let mut manifest: NetsukeManifest = serde_saphyr::from_str(yaml)?;
    for target in &mut manifest.targets {
        if target.name.as_single() == Some("choice") {
            target.conditional = true;
        }
    }
    Ok(manifest)
}

/// Snapshot the v1 graph shape and ensure delayed path tokens are absent.
#[test]
fn graph_v1_schema_snapshot() -> Result<()> {
    let manifest = graph_fixture_manifest()?;
    let graph = graph_json::GraphJson::from_manifest(&manifest);
    let rendered = render_json(&build_catalogue(&manifest), Some(&graph))?;
    ensure!(
        rendered.contains("{{ ins }}") && rendered.contains("{{ outs }}"),
        "export should preserve authored delayed placeholders"
    );
    ensure!(
        !rendered.contains("__NETSUKE_INS_PLACEHOLDER__")
            && !rendered.contains("__NETSUKE_OUTS_PLACEHOLDER__"),
        "internal delayed path tokens must not reach the export"
    );
    help_targets_json_snapshot_settings().bind(|| {
        assert_snapshot!("graph_v1_schema", rendered);
    });
    Ok(())
}
