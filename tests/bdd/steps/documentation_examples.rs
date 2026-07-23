//! Step definitions for examples embedded in user-facing documentation.

use crate::bdd::fixtures::TestWorld;
use crate::documentation_examples::manifest_workspace;
use anyhow::Result;
use rstest_bdd_macros::given;

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
