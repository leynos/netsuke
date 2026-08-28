//! Tests for manifests whose actions or targets only declare dependencies.

use anyhow::{Context, Result, ensure};
use netsuke::{
    ast::{NetsukeManifest, Recipe, StringOrList},
    ir::BuildGraph,
};
use rstest::rstest;
use test_support::display_error_chain;

use super::support::parse_manifest;

#[test]
fn dependency_only_entries_require_non_empty_deps() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        actions:
          - name: all
            deps: [check-fmt, lint]
        targets:
          - name: release
            deps: [all]
    "#;
    let manifest = parse_manifest(yaml)?;
    let action = manifest
        .actions
        .first()
        .context("manifest should contain the dependency-only action")?;
    let target = manifest
        .targets
        .first()
        .context("manifest should contain the dependency-only target")?;
    ensure!(
        matches!(
            &action.recipe,
            Recipe::Command {
                command: StringOrList::Empty
            }
        ) && matches!(
            &target.recipe,
            Recipe::Command {
                command: StringOrList::Empty
            }
        ),
        "entries with deps should deserialize as dependency-only recipes"
    );
    let serialised =
        serde_json::to_value(&manifest).context("serialise dependency-only manifest")?;
    let serialised_action = serialised
        .get("actions")
        .and_then(serde_json::Value::as_array)
        .and_then(|actions| actions.first())
        .context("serialised manifest should contain the dependency-only action")?;
    let serialised_target = serialised
        .get("targets")
        .and_then(serde_json::Value::as_array)
        .and_then(|targets| targets.first())
        .context("serialised manifest should contain the dependency-only target")?;
    ensure!(
        serialised_action.get("command").is_none() && serialised_target.get("command").is_none(),
        "dependency-only entries should serialize without a synthetic command: {serialised}"
    );

    Ok(())
}

/// Reject dependency-only entries without usable dependencies.
#[rstest]
#[case::action_without_dependencies(
    "an action without dependencies",
    r#"
        netsuke_version: "1.0.0"
        actions:
          - name: prepare
        targets: []
    "#
)]
#[case::action_with_empty_dependencies(
    "an action with an empty dependency list",
    r#"
        netsuke_version: "1.0.0"
        actions:
          - name: prepare
            deps: []
        targets: []
    "#
)]
#[case::target_without_dependencies(
    "a target without dependencies",
    r#"
        netsuke_version: "1.0.0"
        targets:
          - name: release
    "#
)]
#[case::target_with_empty_dependencies(
    "a target with an empty dependency list",
    r#"
        netsuke_version: "1.0.0"
        targets:
          - name: release
            deps: []
    "#
)]
#[case::rule_without_recipe(
    "a rule without an executable recipe",
    r#"
        netsuke_version: "1.0.0"
        rules:
          - name: aggregate
        targets:
          - name: release
            rule: aggregate
    "#
)]
#[case::action_with_blank_scalar_dependency(
    "an action with a blank scalar dependency",
    r#"
        netsuke_version: "1.0.0"
        actions:
          - name: prepare
            deps: ""
        targets: []
    "#
)]
#[case::target_with_blank_list_dependency(
    "a target with a blank list dependency",
    r#"
        netsuke_version: "1.0.0"
        targets:
          - name: release
            deps: [""]
    "#
)]
fn dependency_only_entries_without_usable_dependencies_are_rejected(
    #[case] case: &str,
    #[case] invalid_yaml: &str,
) -> Result<()> {
    let error = parse_manifest(invalid_yaml)
        .expect_err("dependency-only entry without usable dependencies should fail");
    let chain = display_error_chain(error.as_ref());
    ensure!(
        chain.contains("missing one of command, script, or rule"),
        "{case} should report the missing-recipe diagnostic: {chain}"
    );
    Ok(())
}

/// Reject dependency-only entries whose rendered dependency is blank.
#[test]
fn rendered_blank_dependency_is_rejected() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        vars:
          aggregate_dep: ""
        actions:
          - name: all
            deps: "{{ aggregate_dep }}"
        targets: []
    "#;
    let error = parse_manifest(yaml).expect_err("a rendered blank dependency should fail");
    let chain = display_error_chain(error.as_ref());
    ensure!(
        chain.contains("missing one of command, script, or rule"),
        "rendered blank dependencies should report the missing-recipe diagnostic: {chain}"
    );
    Ok(())
}

/// Reject invalid direct AST deserialisation before IR lowering.
#[test]
fn direct_ast_deserialisation_is_validated_before_ir_lowering() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        rules:
          - name: aggregate
        targets:
          - name: release
            rule: aggregate
    "#;
    let manifest: NetsukeManifest =
        serde_saphyr::from_str(yaml).context("directly deserialize the invalid manifest")?;
    let error = BuildGraph::from_manifest(&manifest)
        .expect_err("IR lowering should reject a dependency-only rule");
    ensure!(
        error.to_string() == "missing one of command, script, or rule",
        "direct AST lowering should preserve the missing-recipe diagnostic: {error}"
    );
    Ok(())
}
