//! Conditional expansion cases for manifest actions.

use super::*;
use anyhow::{Context, Result};
use minijinja::Environment;
use rstest::rstest;

fn manifest_query_environment() -> Environment<'static> {
    let mut env = Environment::new();
    let _state = crate::stdlib::register_manifest_query(&mut env);
    env
}

#[test]
fn expand_foreach_expands_actions_sequence_values() -> Result<()> {
    let env = Environment::new();
    let yaml = "actions:
  - name: literal
    foreach:
      - alpha
      - beta
    command: echo {{ item }}
    vars:
      static: keep";
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;
    expand_foreach(&mut doc, &env)?;
    let actions = actions(&doc)?;
    anyhow::ensure!(actions.len() == 2, "expected two actions");
    anyhow::ensure!(indexes(actions, "action")? == vec![0, 1], "wrong indexes");
    for action in actions {
        let map = action.as_object().context("action map")?;
        anyhow::ensure!(
            !map.contains_key("foreach"),
            "foreach should be removed after action expansion"
        );
    }
    Ok(())
}

#[test]
fn expand_foreach_applies_action_when_expression() -> Result<()> {
    let env = Environment::new();
    let yaml = "actions:
  - name: literal
    command: echo {{ item }}
    foreach: '[1, 2, 3]'
    when: 'item > 1'";
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;
    expand_foreach(&mut doc, &env)?;
    let actions = actions(&doc)?;
    anyhow::ensure!(actions.len() == 2, "expected filtered actions");
    anyhow::ensure!(indexes(actions, "action")? == vec![1, 2], "wrong indexes");
    ensure_foreach_removed(actions, "filtered action")?;
    Ok(())
}

#[test]
fn expand_static_action_when_false_drops_action() -> Result<()> {
    let env = Environment::new();
    let yaml = "actions:
  - name: skipped
    command: echo skipped
    when: 'false'
  - name: kept
    command: echo kept";
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;
    expand_foreach(&mut doc, &env)?;
    let actions = actions(&doc)?;
    anyhow::ensure!(actions.len() == 1, "expected one action");
    let map = actions
        .first()
        .and_then(ManifestValue::as_object)
        .context("action map")?;
    let name = map
        .get("name")
        .and_then(ManifestValue::as_str)
        .context("action name")?;
    anyhow::ensure!(name == "kept", "unexpected action name: {name}");
    anyhow::ensure!(
        !map.contains_key("when"),
        "when should be removed after action expansion"
    );
    Ok(())
}

#[rstest]
#[case::present("preferred-tool", "preferred")]
#[case::absent("missing-tool", "fallback")]
fn expand_static_action_when_supports_complementary_command_available_branches(
    #[case] command_name: &str,
    #[case] expected_name: &str,
) -> Result<()> {
    let mut env = Environment::new();
    env.add_function("command_available", |name: String| {
        Ok::<bool, minijinja::Error>(name == "preferred-tool")
    });
    let yaml = format!(
        "actions:
  - name: preferred
    command: echo preferred
    when: command_available({command_name:?})
  - name: fallback
    command: echo fallback
    when: not command_available({command_name:?})"
    );
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;
    expand_foreach(&mut doc, &env)?;
    let actions = actions(&doc)?;
    anyhow::ensure!(actions.len() == 1, "expected exactly one action branch");
    let map = actions
        .first()
        .and_then(ManifestValue::as_object)
        .context("action map")?;
    let name = map
        .get("name")
        .and_then(ManifestValue::as_str)
        .context("action name")?;
    anyhow::ensure!(name == expected_name, "unexpected action branch: {name}");
    anyhow::ensure!(
        !map.contains_key("when"),
        "when should be removed after action expansion"
    );
    Ok(())
}

#[test]
fn manifest_query_keeps_complementary_command_available_actions_conditional() -> Result<()> {
    let env = manifest_query_environment();
    let yaml = "actions:
  - name: preferred
    description: Use cargo-nextest when installed
    command: cargo nextest run
    when: command_available('cargo-nextest')
  - name: fallback
    description: Use Cargo otherwise
    command: cargo test
    when: not command_available('cargo-nextest')";
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;

    expand_foreach(&mut doc, &env)?;

    let actions = actions(&doc)?;
    anyhow::ensure!(
        actions.len() == 2,
        "query should retain both action branches"
    );
    for action in actions {
        let map = action.as_object().context("conditional action map")?;
        anyhow::ensure!(
            map.get("conditional") == Some(&ManifestValue::Bool(true)),
            "query-disabled action should be marked conditional: {map:?}"
        );
        anyhow::ensure!(
            map.get("description")
                .and_then(ManifestValue::as_str)
                .is_some(),
            "conditional action should retain its description"
        );
    }
    Ok(())
}

#[test]
fn manifest_query_still_filters_ordinary_false_foreach_actions() -> Result<()> {
    let env = manifest_query_environment();
    let yaml = "actions:
  - name: test-{{ item }}
    command: cargo test
    foreach: [skip, keep]
    when: item != 'skip'";
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;

    expand_foreach(&mut doc, &env)?;

    let actions = actions(&doc)?;
    anyhow::ensure!(
        actions.len() == 1,
        "ordinary false branch should be filtered"
    );
    let action = actions
        .first()
        .and_then(ManifestValue::as_object)
        .context("kept action map")?;
    anyhow::ensure!(
        !action.contains_key("conditional"),
        "ordinary true branch should not be marked conditional"
    );
    Ok(())
}

#[test]
fn manifest_query_propagates_unrelated_when_errors() -> Result<()> {
    let env = manifest_query_environment();
    let yaml = "actions:
  - name: broken
    command: cargo test
    when: unknown_helper()";
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;

    let error = expand_foreach(&mut doc, &env)
        .err()
        .context("unknown helpers should remain manifest errors")?;
    anyhow::ensure!(
        format!("{error:#}").contains("unknown_helper"),
        "unexpected unrelated when error: {error:#}"
    );
    Ok(())
}
