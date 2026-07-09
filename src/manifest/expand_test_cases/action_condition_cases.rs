//! Conditional expansion cases for manifest actions.

use super::*;
use anyhow::{Context, Result};
use minijinja::Environment;
use rstest::rstest;

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
