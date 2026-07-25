//! Step definitions for examples embedded in user-facing documentation.

use crate::bdd::fixtures::TestWorld;
use crate::documentation_examples::manifest_workspace;
use anyhow::{Context, Result, ensure};
use rstest_bdd_macros::{given, then};
use test_support::fs as test_fs;

/// Create a workspace from the exact YAML fence identified in the docs.
#[given("a workspace from documentation example {id:string}")]
fn workspace_from_documentation_example(world: &TestWorld, id: String) -> Result<()> {
    let workspace = manifest_workspace(&id)?;
    *world.workspace_path.borrow_mut() = Some(workspace.path().to_path_buf());
    *world.temp_dir.borrow_mut() = Some(workspace);
    world.run_status.clear();
    world.run_error.clear();
    world.command_stdout.clear();
    world.command_stderr.clear();
    Ok(())
}

#[then("the documentation file {path:string} should contain {contents:string}")]
fn documentation_file_contains(world: &TestWorld, path: String, contents: String) -> Result<()> {
    let workspace_slot = world.workspace_path.borrow();
    let workspace = workspace_slot
        .as_ref()
        .context("documentation workspace has not been initialized")?;
    let rendered = test_fs::read_to_string(workspace.join(&path))
        .with_context(|| format!("read documentation artefact {path}"))?;
    ensure!(
        rendered.contains(&contents),
        "expected documentation artefact {path} to contain {contents:?}, got {rendered:?}"
    );
    Ok(())
}
