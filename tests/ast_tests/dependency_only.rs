//! Tests for manifests whose actions or targets only declare dependencies.

use anyhow::{Context, Result, ensure};
use netsuke::ast::Recipe;

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
        matches!(action.recipe, Recipe::DependencyOnly)
            && matches!(target.recipe, Recipe::DependencyOnly),
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

    for invalid_yaml in [
        r#"
            netsuke_version: "1.0.0"
            targets:
              - name: release
        "#,
        r#"
            netsuke_version: "1.0.0"
            targets:
              - name: release
                deps: []
        "#,
    ] {
        ensure!(
            parse_manifest(invalid_yaml).is_err(),
            "a dependency-only entry without deps should fail: {invalid_yaml}"
        );
    }
    Ok(())
}
