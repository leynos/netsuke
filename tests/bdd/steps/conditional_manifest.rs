//! Step definitions for conditional manifest planning scenarios.

use crate::bdd::fixtures::TestWorld;
use anyhow::{Context, Result};
use rstest_bdd_macros::given;
use std::fs;

const CONDITIONAL_MANIFEST: &str = r#"netsuke_version: "1.0.0"
actions:
  - foreach:
      - kept
      - skipped
    when: item != 'skipped'
    name: "action-{{ item }}"
    command: "echo action-{{ item }}"
targets:
  - foreach:
      - kept
      - skipped
    when: item != 'skipped'
    name: "target-{{ item }}"
    command: "echo target-{{ item }}"
"#;

/// Create a workspace whose manifest contains conditional actions and targets.
#[given("a Netsuke workspace with conditional actions and targets")]
fn conditional_actions_and_targets_workspace(world: &TestWorld) -> Result<()> {
    let temp = tempfile::tempdir().context("create temp dir for conditional manifest")?;
    let netsukefile = temp.path().join("Netsukefile");
    fs::write(&netsukefile, CONDITIONAL_MANIFEST)
        .with_context(|| format!("write manifest to {}", netsukefile.display()))?;
    *world.temp_dir.borrow_mut() = Some(temp);
    world.run_status.clear();
    world.run_error.clear();
    world.command_stdout.clear();
    world.command_stderr.clear();
    Ok(())
}
