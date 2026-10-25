//! Tests for manifests whose actions or targets only declare dependencies.

use anyhow::{Context, Result, ensure};
use netsuke::localization::{self, keys};
use netsuke::{
    ast::{CommandIsolation, NetsukeManifest, Recipe, StringOrList},
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
                command: StringOrList::Empty,
                isolation: CommandIsolation::Shared,
                ..
            }
        ) && matches!(
            &target.recipe,
            Recipe::Command {
                command: StringOrList::Empty,
                isolation: CommandIsolation::Shared,
                ..
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
    let serialised_action_recipe = serialised_action
        .get("Command")
        .and_then(serde_json::Value::as_object)
        .context("serialised dependency-only action should contain its recipe")?;
    let serialised_target_recipe = serialised_target
        .get("Command")
        .and_then(serde_json::Value::as_object)
        .context("serialised dependency-only target should contain its recipe")?;
    ensure!(
        serialised_action_recipe.get("command").is_none()
            && serialised_target_recipe.get("command").is_none()
            && serialised_action_recipe.get("command_isolation").is_none()
            && serialised_target_recipe.get("command_isolation").is_none(),
        "dependency-only entries should serialize without a synthetic command: {serialised}"
    );

    Ok(())
}

/// Reject a mode-only entry instead of treating it as dependency-only.
#[test]
fn command_isolation_without_a_recipe_is_rejected() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        actions:
          - name: all
            deps: [check-fmt]
            command_isolation: shared
        targets: []
    "#;
    let error = parse_manifest(yaml).expect_err("mode-only entries are not dependency-only");
    let chain = display_error_chain(error.as_ref());
    let expected = localization::message(keys::MANIFEST_COMMAND_ISOLATION_RECIPE).to_string();
    ensure!(
        chain.contains(&expected),
        "unexpected mode-only diagnostic: {chain}"
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

/// Reject console scheduling on dependency-only entries during parsing.
#[rstest]
#[case::action("actions:\n  - name: all\n    deps: [lint]\n    console: true\ntargets: []\n")]
#[case::target("actions: []\ntargets:\n  - name: release\n    deps: [build]\n    console: true\n")]
fn dependency_only_console_entries_are_rejected(#[case] sections: &str) -> Result<()> {
    let yaml = format!("netsuke_version: '1.0.0'\n{sections}");
    let error =
        parse_manifest(&yaml).expect_err("dependency-only entries cannot use the console pool");
    let chain = display_error_chain(error.as_ref());
    ensure!(
        chain.contains("console cannot be enabled for dependency-only entries"),
        "unexpected console dependency-only diagnostic: {chain}"
    );
    Ok(())
}

/// Preserve the console-specific validation diagnostic during direct IR lowering.
#[test]
fn direct_ir_lowering_preserves_console_dependency_only_diagnostic() -> Result<()> {
    let yaml = concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: release\n",
        "    deps: [build]\n",
        "    console: true\n",
    );
    let manifest: NetsukeManifest =
        serde_saphyr::from_str(yaml).context("directly deserialize the manifest")?;
    let error = BuildGraph::from_manifest(&manifest)
        .expect_err("console scheduling on an aggregate should fail IR lowering");
    ensure!(
        error.to_string() == "console cannot be enabled for dependency-only entries",
        "direct AST lowering should preserve the console diagnostic: {error}"
    );
    Ok(())
}
