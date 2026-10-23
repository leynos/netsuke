//! End-to-end contracts for the opt-in `help targets --edges` JSON export.

use super::*;
use serde_json::Value;

#[path = "edges/cli.rs"]
mod cli;

#[path = "edges/assertions.rs"]
mod assertions;
use assertions::{
    assert_action_recipes, assert_catalogue_and_defaults, assert_conditional_alternatives,
    assert_recipes_not_run, assert_target_graph_metadata,
};

const EDGE_EXPORT_MANIFEST: &str = r#"netsuke_version: "1.0.0"
rules:
  - name: compile
    description: Compile source files
    command: 'echo {{ fetch("https://example.invalid/source", cache=true) }} {{ ins }} {{ outs }}'
  - name: helper-script
    script: |
      echo {{ shell("touch shell-ran") }}
actions:
  - name: [check, verify]
    description: Check the source tree
    command: 'echo {{ command_available("compiler") }} {{ env("NETSUKE_GRAPH_EXPORT_SECRET") }} {{ "ignored" | shell("touch shell-ran") }} {{ ins }} {{ outs }}'
  - name: prepare
    command_isolation: isolated
    command:
      - 'echo {{ ins }}'
      - 'echo {{ outs }}'
  - name: conditional-check
    description: Preferred conditional action
    command: touch conditional-ran
    when: command_available("netsuke-graph-export-missing")
  - name: conditional-check
    description: Fallback conditional action
    command: touch conditional-ran
    when: not command_available("netsuke-graph-export-missing")
targets:
  - {name: input, command: 'touch input-ran'}
  - name: ready
    command: touch ready-ran
  - name: serial
    description: Run the serial gate
    script: |
      echo {{ ins }} {{ outs }}
    sources: [source.c]
    deps: [input]
    order_only_deps: [ready]
    dependency_order: serial
    always: true
  - name: built
    rule: compile
  - name: aggregate
    deps: [built]
defaults: [check, built, built]
"#;

/// Write a manifest into a temporary workspace and retain its capability.
fn edge_export_workspace(manifest: &str) -> Result<(tempfile::TempDir, Utf8PathBuf, Dir)> {
    let temp = tempfile::tempdir().context("create edge-export workspace")?;
    let temp_path = Utf8Path::from_path(temp.path()).context("workspace path should be UTF-8")?;
    let manifest_path = temp_path.join("Netsukefile");
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open edge-export workspace")?;
    workspace
        .write("Netsukefile", manifest.as_bytes())
        .context("write edge-export manifest")?;
    Ok((temp, manifest_path, workspace))
}

/// Run the graph-bearing help query against one isolated manifest.
fn run_edge_export(
    manifest_path: &Utf8Path,
    current_dir: &Utf8Path,
) -> Result<std::process::Output> {
    assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(current_dir)
        .env("NETSUKE_NINJA", "/definitely-not-a-ninja-binary")
        .arg("--json")
        .arg("--file")
        .arg(manifest_path)
        .args(["help", "targets", "--edges"])
        .output()
        .context("run netsuke --json help targets --edges")
}

/// Export raw recipes and prove declaration and edge metadata match the catalogue.
#[test]
fn graph_export_is_hermetic_and_matches_catalogue_rows() -> Result<()> {
    let (temp, manifest_path, workspace) = edge_export_workspace(EDGE_EXPORT_MANIFEST)?;
    let temp_path = Utf8Path::from_path(temp.path()).context("workspace path should be UTF-8")?;
    let output = run_edge_export(&manifest_path, temp_path)?;
    ensure!(
        output.status.success(),
        "graph export should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).context("graph JSON should be UTF-8")?;
    let document: Value = serde_json::from_str(&stdout).context("parse graph JSON result")?;
    let result = document
        .get("result")
        .context("result envelope should exist")?;
    let graph = result
        .get("graph")
        .context("edges option should attach the graph projection")?;
    let nodes = graph
        .get("nodes")
        .and_then(Value::as_array)
        .context("graph nodes should be an array")?;
    assert_catalogue_and_defaults(result, graph, nodes)?;
    assert_action_recipes(nodes)?;
    assert_target_graph_metadata(graph, nodes)?;
    assert_conditional_alternatives(nodes)?;
    assert_recipes_not_run(&workspace)
}

/// Reject impure helpers in structural metadata without writing partial JSON.
#[rstest]
#[case::name("name", "name: '{{ env(\"PATH\") }}'\n")]
#[case::description(
    "description",
    "name: stable\n    description: '{{ env(\"PATH\") }}'\n"
)]
fn graph_export_rejects_disabled_helpers_in_metadata(
    #[case] field: &str,
    #[case] metadata: &str,
) -> Result<()> {
    let manifest =
        format!("netsuke_version: \"1.0.0\"\ntargets:\n  - {metadata}    command: echo hi\n");
    let (temp, manifest_path, workspace) = edge_export_workspace(&manifest)?;
    let temp_path = Utf8Path::from_path(temp.path()).context("workspace path should be UTF-8")?;
    let output = run_edge_export(&manifest_path, temp_path)?;

    ensure!(
        !output.status.success(),
        "impure {field} should fail export"
    );
    ensure!(
        output.stdout.is_empty(),
        "failed graph export must not write partial JSON"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("env is disabled"),
        "impure {field} should fail because of the disabled env helper: {stderr}"
    );
    assert_recipes_not_run(&workspace)
}
