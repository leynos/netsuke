//! Reusable assertions for the hermetic graph-export integration tests.

use anyhow::{Context, Result, ensure};
use cap_std::fs_utf8::Dir;
use serde_json::Value;
use std::collections::HashSet;

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
pub(super) fn assert_recipes_not_run(workspace: &Dir) -> Result<()> {
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
pub(super) fn assert_catalogue_and_defaults(
    result: &Value,
    graph: &Value,
    nodes: &[Value],
) -> Result<()> {
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
pub(super) fn assert_action_recipes(nodes: &[Value]) -> Result<()> {
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
    ensure!(
        check.pointer("/recipe/isolation").and_then(Value::as_str) == Some("shared"),
        "string commands should report shared isolation"
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
    ensure!(
        prepare.pointer("/recipe/isolation").and_then(Value::as_str) == Some("isolated"),
        "list commands should preserve their isolated mode"
    );
    Ok(())
}

/// Check each authored edge class and the selected dependency order.
fn assert_target_dependency_edges_and_ordering(nodes: &[Value]) -> Result<()> {
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
    Ok(())
}

/// Check that target script placeholders remain unrendered.
fn assert_raw_target_scripts(nodes: &[Value]) -> Result<()> {
    let serial = graph_node(nodes, "serial")?;
    ensure!(
        serial.pointer("/recipe/kind").and_then(Value::as_str) == Some("script")
            && serial
                .pointer("/recipe/script")
                .and_then(Value::as_str)
                .is_some_and(|script| script.contains("{{ ins }}") && script.contains("{{ outs }}")),
        "script bodies should remain raw"
    );
    Ok(())
}

/// Check rule selectors and raw command and script recipes.
fn assert_rule_references_and_raw_recipes(graph: &Value, nodes: &[Value]) -> Result<()> {
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
        graph
            .pointer("/rules/0/recipe/isolation")
            .and_then(Value::as_str)
            == Some("shared"),
        "rule commands should retain their default isolation mode"
    );
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
    Ok(())
}

/// Check the closed recipe kind used by targets with dependencies only.
fn assert_dependency_only_recipes(nodes: &[Value]) -> Result<()> {
    ensure!(
        graph_node(nodes, "aggregate")?
            .pointer("/recipe/kind")
            .and_then(Value::as_str)
            == Some("dependency_only"),
        "dependency-only targets should use their closed recipe kind"
    );
    Ok(())
}

/// Check each independent target graph metadata contract.
pub(super) fn assert_target_graph_metadata(graph: &Value, nodes: &[Value]) -> Result<()> {
    assert_target_dependency_edges_and_ordering(nodes)?;
    assert_raw_target_scripts(nodes)?;
    assert_rule_references_and_raw_recipes(graph, nodes)?;
    assert_dependency_only_recipes(nodes)
}

/// Preserve all alternatives whose conditions cannot be evaluated by the query.
pub(super) fn assert_conditional_alternatives(nodes: &[Value]) -> Result<()> {
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
