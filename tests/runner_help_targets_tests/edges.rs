//! End-to-end contracts for the opt-in `help targets --edges` JSON export.

use super::*;
use serde_json::Value;
use std::collections::HashSet;

#[path = "edges/cli.rs"]
mod cli;

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
  - name: input
    command: touch input-ran
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

/// Find the declaration node containing one resolved name.
fn graph_node<'a>(nodes: &'a [Value], name: &str) -> Result<&'a Value> {
    nodes
        .iter()
        .find(|node| {
            node.pointer("/names")
                .and_then(Value::as_array)
                .is_some_and(|names| {
                    names
                        .iter()
                        .any(|entry| entry.pointer("/name").and_then(Value::as_str) == Some(name))
                })
        })
        .with_context(|| format!("graph should contain declaration name {name:?}"))
}

/// Flatten graph names for one kind in declaration expansion order.
fn graph_names(nodes: &[Value], kind: &str) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for node in nodes
        .iter()
        .filter(|node| node.pointer("/kind").and_then(Value::as_str) == Some(kind))
    {
        for name in node
            .pointer("/names")
            .and_then(Value::as_array)
            .context("node names should be an array")?
        {
            names.push(
                name.pointer("/name")
                    .and_then(Value::as_str)
                    .context("node name should be a string")?
                    .to_owned(),
            );
        }
    }
    Ok(names)
}

/// Read catalogue row names in their serialized order.
fn catalogue_names(result: &Value, section: &str) -> Result<Vec<String>> {
    result
        .get(section)
        .and_then(Value::as_array)
        .context("catalogue section should be an array")?
        .iter()
        .map(|entry| {
            entry
                .pointer("/name")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .context("catalogue row name should be a string")
        })
        .collect()
}

/// Confirm no recipe output or query-state directory was created.
fn assert_recipes_not_run(workspace: &Dir) -> Result<()> {
    for output in [
        "shell-ran",
        "conditional-ran",
        "input-ran",
        "ready-ran",
        ".netsuke",
    ] {
        ensure!(
            !workspace.try_exists(output)?,
            "graph export must not create {output}"
        );
    }
    Ok(())
}

/// Match graph names against catalogue rows and authored defaults.
fn assert_catalogue_and_defaults(result: &Value, graph: &Value, nodes: &[Value]) -> Result<()> {
    for (kind, section) in [("action", "actions"), ("target", "targets")] {
        ensure!(
            graph_names(nodes, kind)? == catalogue_names(result, section)?,
            "{kind} nodes should flatten to catalogue names in declaration order"
        );
    }

    let defaults = graph
        .get("defaults")
        .and_then(Value::as_array)
        .context("graph defaults should be an array")?;
    let authored_defaults: Vec<&str> = defaults.iter().filter_map(Value::as_str).collect();
    ensure!(
        authored_defaults == ["check", "built", "built"],
        "graph should preserve default order and duplicates: {authored_defaults:?}"
    );
    let default_names: HashSet<&str> = defaults.iter().filter_map(Value::as_str).collect();
    for node in nodes {
        for name in node
            .pointer("/names")
            .and_then(Value::as_array)
            .context("node names should be an array")?
        {
            let resolved = name
                .pointer("/name")
                .and_then(Value::as_str)
                .context("node name should be a string")?;
            ensure!(
                name.pointer("/default").and_then(Value::as_bool)
                    == Some(default_names.contains(resolved)),
                "default marker for {resolved:?} should match authored defaults"
            );
        }
    }
    Ok(())
}

/// Check action command variants and raw helper calls.
fn assert_action_recipes(nodes: &[Value]) -> Result<()> {
    let check = graph_node(nodes, "check")?;
    ensure!(
        check.pointer("/names/1/name").and_then(Value::as_str) == Some("verify"),
        "multi-name declarations should remain one node with ordered names"
    );
    let action_command = check
        .pointer("/recipe/commands/0")
        .and_then(Value::as_str)
        .context("action command should retain its raw string")?;
    for helper in [
        "command_available",
        "env(",
        "shell(",
        "{{ ins }}",
        "{{ outs }}",
    ] {
        ensure!(
            action_command.contains(helper),
            "raw command should retain {helper:?}: {action_command}"
        );
    }
    ensure!(
        check.pointer("/recipe/kind").and_then(Value::as_str) == Some("command")
            && check.pointer("/recipe/form").and_then(Value::as_str) == Some("string"),
        "string commands should carry their source form"
    );
    let prepare = graph_node(nodes, "prepare")?;
    ensure!(
        prepare.pointer("/recipe/form").and_then(Value::as_str) == Some("list")
            && prepare
                .pointer("/recipe/commands")
                .and_then(Value::as_array)
                .is_some_and(|commands| commands.len() == 2),
        "list commands should retain their form and entry order"
    );
    Ok(())
}

/// Check edge classes, ordering, scripts, rules, and dependency-only recipes.
fn assert_target_graph_metadata(graph: &Value, nodes: &[Value]) -> Result<()> {
    let serial = graph_node(nodes, "serial")?;
    ensure!(
        serial.pointer("/dependency_order").and_then(Value::as_str) == Some("serial")
            && serial.pointer("/edges/sources/0").and_then(Value::as_str) == Some("source.c")
            && serial.pointer("/edges/deps/0").and_then(Value::as_str) == Some("input")
            && serial
                .pointer("/edges/order_only_deps/0")
                .and_then(Value::as_str)
                == Some("ready"),
        "serial nodes should preserve each authored edge kind"
    );
    ensure!(
        serial.pointer("/recipe/kind").and_then(Value::as_str) == Some("script")
            && serial
                .pointer("/recipe/script")
                .and_then(Value::as_str)
                .is_some_and(|script| script.contains("{{ ins }}") && script.contains("{{ outs }}")),
        "script bodies should remain raw"
    );
    ensure!(
        graph_node(nodes, "built")?
            .pointer("/recipe/names/0")
            .and_then(Value::as_str)
            == Some("compile"),
        "rule selectors should be serialized as names"
    );
    let compile_rule = graph
        .pointer("/rules/0/recipe/commands/0")
        .and_then(Value::as_str)
        .context("rule command should be present")?;
    ensure!(
        compile_rule.contains("{{ fetch(")
            && compile_rule.contains("{{ ins }}")
            && compile_rule.contains("{{ outs }}"),
        "rule commands should preserve fetch and path placeholders"
    );
    let helper_script = graph
        .pointer("/rules/1/recipe/script")
        .and_then(Value::as_str)
        .context("rule script should be present")?;
    ensure!(
        helper_script.contains("{{ shell(") && helper_script.contains("touch shell-ran"),
        "rule scripts should preserve disabled helper calls verbatim"
    );
    ensure!(
        graph_node(nodes, "aggregate")?
            .pointer("/recipe/kind")
            .and_then(Value::as_str)
            == Some("dependency_only"),
        "dependency-only targets should use their closed recipe kind"
    );
    Ok(())
}

/// Preserve all alternatives whose conditions cannot be evaluated by the query.
fn assert_conditional_alternatives(nodes: &[Value]) -> Result<()> {
    let conditional_nodes: Vec<&Value> = nodes
        .iter()
        .filter(|node| {
            node.pointer("/names")
                .and_then(Value::as_array)
                .is_some_and(|names| {
                    names.iter().any(|name| {
                        name.pointer("/name").and_then(Value::as_str) == Some("conditional-check")
                    })
                })
        })
        .collect();
    ensure!(
        conditional_nodes.len() == 2
            && conditional_nodes.iter().all(|node| {
                node.pointer("/conditional").and_then(Value::as_bool) == Some(true)
            }),
        "same-name alternatives using disabled helpers should remain conditional"
    );
    Ok(())
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
#[case::name("name")]
#[case::description("description")]
fn graph_export_rejects_disabled_helpers_in_metadata(#[case] field: &str) -> Result<()> {
    let metadata = match field {
        "name" => "name: '{{ env(\"PATH\") }}'\n".to_owned(),
        "description" => "name: stable\n    description: '{{ env(\"PATH\") }}'\n".to_owned(),
        _ => anyhow::bail!("unexpected metadata field {field:?}"),
    };
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
    assert_recipes_not_run(&workspace)
}
