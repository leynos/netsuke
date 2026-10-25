//! Snapshot the graph export schema and its raw recipe boundary.

use super::super::{build_catalogue, graph_json, graph_json::GraphJson, render_json};
use crate::ast::{
    CommandIsolation, DependencyOrder, NetsukeManifest, Recipe, Rule, StringOrList, Target,
};
use crate::snapshot_test_support::help_targets_json_snapshot_settings;
use anyhow::{Result, ensure};
use insta::assert_snapshot;
use proptest::prelude::*;
use serde_json::{Value, json};
use std::collections::HashMap;

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
    command_isolation: isolated
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

/// Generate short names from a small domain so duplicates occur regularly.
fn manifest_string() -> impl Strategy<Value = String> {
    (0_u8..4).prop_map(|value| format!("name-{value}"))
}

/// Generate raw recipe text, including delayed placeholders used by builds.
fn raw_recipe_text() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("{{ ins }}".to_owned()),
        Just("{{ outs }}".to_owned()),
        Just("{{ command_available(\"compiler\") }}".to_owned()),
        "[a-z ]{0,16}".prop_map(std::convert::identity),
    ]
}

/// Generate scalar, list, and empty manifest string fields.
fn string_or_list_strategy() -> impl Strategy<Value = StringOrList> {
    prop_oneof![
        Just(StringOrList::Empty),
        manifest_string().prop_map(StringOrList::String),
        proptest::collection::vec(manifest_string(), 0..5).prop_map(StringOrList::List),
    ]
}

/// Generate executable recipes with both command-list state policies.
fn executable_recipe_strategy() -> impl Strategy<Value = Recipe> {
    let string_command = raw_recipe_text().prop_map(|command| Recipe::Command {
        command: StringOrList::String(command),
        isolation: CommandIsolation::Shared,
    });
    let list_command = (
        proptest::collection::vec(raw_recipe_text(), 0..5),
        any::<bool>(),
    )
        .prop_map(|(commands, isolated)| Recipe::Command {
            command: StringOrList::List(commands),
            isolation: if isolated {
                CommandIsolation::Isolated
            } else {
                CommandIsolation::Shared
            },
        });

    prop_oneof![
        string_command,
        list_command,
        raw_recipe_text().prop_map(|script| Recipe::Script { script }),
        string_or_list_strategy().prop_map(|rule| Recipe::Rule { rule }),
    ]
}

/// Generate target recipes, including the internal dependency-only marker.
fn recipe_strategy() -> impl Strategy<Value = Recipe> {
    prop_oneof![
        executable_recipe_strategy(),
        Just(Recipe::Command {
            command: StringOrList::Empty,
            isolation: CommandIsolation::Shared,
        }),
    ]
}

/// Generate one declaration with bounded names, edge lists, and flags.
fn target_strategy() -> impl Strategy<Value = Target> {
    (
        string_or_list_strategy(),
        recipe_strategy(),
        string_or_list_strategy(),
        string_or_list_strategy(),
        string_or_list_strategy(),
        prop_oneof![
            Just(DependencyOrder::Parallel),
            Just(DependencyOrder::Serial),
        ],
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        proptest::option::of(manifest_string()),
    )
        .prop_map(
            |(
                name,
                recipe,
                sources,
                deps,
                order_only_deps,
                dependency_order,
                phony,
                always,
                conditional,
                description,
            )| Target {
                name,
                recipe,
                sources,
                deps,
                dependency_order,
                order_only_deps,
                vars: HashMap::default(),
                phony,
                always,
                console: false,
                conditional,
                description,
            },
        )
}

/// Generate rule declarations in an order with duplicate names permitted.
fn rule_strategy() -> impl Strategy<Value = Rule> {
    (
        manifest_string(),
        proptest::option::of(manifest_string()),
        executable_recipe_strategy(),
    )
        .prop_map(|(name, description, recipe)| Rule {
            name,
            recipe,
            description,
        })
}

/// Generate a bounded manifest with duplicate values and varied declarations.
fn manifest_strategy() -> impl Strategy<Value = NetsukeManifest> {
    (
        proptest::collection::vec(rule_strategy(), 0..5),
        proptest::collection::vec(target_strategy(), 0..6),
        proptest::collection::vec(target_strategy(), 0..6),
        proptest::collection::vec(manifest_string(), 0..8),
    )
        .prop_map(|(rules, actions, targets, defaults)| NetsukeManifest {
            netsuke_version: semver::Version::new(1, 0, 0),
            vars: HashMap::default(),
            macros: Vec::new(),
            rules,
            actions,
            targets,
            defaults,
        })
}

/// Flatten a manifest string field in the way its authored form requires.
fn expected_strings(strings: &StringOrList) -> Vec<String> {
    match strings {
        StringOrList::Empty => Vec::new(),
        StringOrList::String(value) => vec![value.clone()],
        StringOrList::List(values) => values.clone(),
    }
}

/// Build the independent JSON expectation for one recipe declaration.
fn expected_recipe(recipe: &Recipe) -> Value {
    match recipe {
        Recipe::Command {
            command: StringOrList::String(command),
            isolation,
        } => json!({
            "kind": "command",
            "form": "string",
            "commands": [command],
            "isolation": expected_isolation(*isolation),
        }),
        Recipe::Command {
            command: StringOrList::List(commands),
            isolation,
        } => json!({
            "kind": "command",
            "form": "list",
            "commands": commands,
            "isolation": expected_isolation(*isolation),
        }),
        Recipe::Command {
            command: StringOrList::Empty,
            ..
        } => json!({"kind": "dependency_only"}),
        Recipe::Script { script } => json!({"kind": "script", "script": script}),
        Recipe::Rule { rule } => json!({"kind": "rule", "names": expected_strings(rule)}),
    }
}

/// Map command isolation independently into its JSON spelling.
const fn expected_isolation(isolation: CommandIsolation) -> &'static str {
    match isolation {
        CommandIsolation::Shared => "shared",
        CommandIsolation::Isolated => "isolated",
    }
}

/// Build the independent JSON expectation for one declaration node.
fn expected_node(target: &Target, kind: &str, defaults: &[String]) -> Value {
    let names = expected_strings(&target.name)
        .into_iter()
        .map(|name| {
            json!({
                "name": name,
                "default": defaults.iter().any(|default| default == &name),
            })
        })
        .collect::<Vec<_>>();
    let dependency_order = match target.dependency_order {
        DependencyOrder::Parallel => "parallel",
        DependencyOrder::Serial => "serial",
    };
    json!({
        "kind": kind,
        "names": names,
        "description": &target.description,
        "conditional": target.conditional,
        "phony": target.phony,
        "always": target.always,
        "dependency_order": dependency_order,
        "edges": {
            "sources": expected_strings(&target.sources),
            "deps": expected_strings(&target.deps),
            "order_only_deps": expected_strings(&target.order_only_deps),
        },
        "recipe": expected_recipe(&target.recipe),
    })
}

/// Build the independent JSON expectation for one rule declaration.
fn expected_rule(rule: &Rule) -> Value {
    json!({
        "name": &rule.name,
        "description": &rule.description,
        "recipe": expected_recipe(&rule.recipe),
    })
}

/// Compare the complete projection against generated authored declarations.
fn expected_graph(manifest: &NetsukeManifest) -> Value {
    let rules = manifest.rules.iter().map(expected_rule).collect::<Vec<_>>();
    let mut nodes = manifest
        .actions
        .iter()
        .map(|target| expected_node(target, "action", &manifest.defaults))
        .collect::<Vec<_>>();
    nodes.extend(
        manifest
            .targets
            .iter()
            .map(|target| expected_node(target, "target", &manifest.defaults)),
    );
    json!({
        "defaults": &manifest.defaults,
        "rules": rules,
        "nodes": nodes,
    })
}

proptest! {
    #[test]
    fn graph_projection_preserves_generated_manifest_order_and_content(
        manifest in manifest_strategy()
    ) {
        let actual = serde_json::to_value(GraphJson::from_manifest(&manifest))
            .expect("graph projection should serialize");
        prop_assert_eq!(actual, expected_graph(&manifest));
    }
}
