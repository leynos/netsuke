//! Target-level `command_available` expansion cases.

use super::{expand_foreach, targets};
use crate::manifest::ManifestValue;
use anyhow::{Context, Result};
use minijinja::Environment;
use rstest::rstest;

#[rstest]
#[case::present("preferred-tool", "preferred")]
#[case::absent("missing-tool", "fallback")]
fn expand_static_target_when_supports_complementary_command_available_branches(
    #[case] command_name: &str,
    #[case] expected_name: &str,
) -> Result<()> {
    let mut env = Environment::new();
    env.add_function("command_available", |name: String| {
        Ok::<bool, minijinja::Error>(name == "preferred-tool")
    });
    let yaml = format!(
        "targets:
  - name: preferred
    command: echo preferred
    when: command_available({command_name:?})
  - name: fallback
    command: echo fallback
    when: not command_available({command_name:?})"
    );
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;

    expand_foreach(&mut doc, &env)?;

    let targets = targets(&doc)?;
    anyhow::ensure!(targets.len() == 1, "expected exactly one target branch");
    let map = targets
        .first()
        .and_then(ManifestValue::as_object)
        .context("target map")?;
    let name = map
        .get("name")
        .and_then(ManifestValue::as_str)
        .context("target name")?;
    anyhow::ensure!(name == expected_name, "unexpected target branch: {name}");
    anyhow::ensure!(
        !map.contains_key("when"),
        "when should be removed after target expansion"
    );
    Ok(())
}
