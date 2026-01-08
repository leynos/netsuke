//! Step definitions for `netsuke manifest` behavioural tests.

use crate::bdd::fixtures::TestWorld;
use crate::bdd::helpers::assertions::assert_slot_contains;
use crate::bdd::types::{DirectoryName, FileName, ManifestOutputPath, OutputFragment};
use anyhow::{Context, Result, ensure};
use rstest_bdd::Slot;
use rstest_bdd_macros::{given, then, when};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use test_support::netsuke::run_netsuke_in;

/// Type of output stream for assertions.
#[derive(Copy, Clone)]
enum OutputType {
    Stdout,
    Stderr,
}

impl fmt::Display for OutputType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stdout => write!(f, "stdout"),
            Self::Stderr => write!(f, "stderr"),
        }
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn get_temp_path(world: &TestWorld) -> Result<PathBuf> {
    let temp = world.temp_dir.borrow();
    let dir = temp.as_ref().context("temp dir has not been initialised")?;
    Ok(dir.path().to_path_buf())
}

fn assert_output_contains(
    output: &Slot<String>,
    output_type: OutputType,
    fragment: &OutputFragment,
) -> Result<()> {
    assert_slot_contains(output, fragment.as_str(), &output_type.to_string())
}

fn assert_file_existence(world: &TestWorld, name: &FileName, should_exist: bool) -> Result<()> {
    let temp_path = get_temp_path(world)?;
    let path = temp_path.join(name.as_str());
    let expected = if should_exist { "exist" } else { "not exist" };
    ensure!(
        path.exists() == should_exist,
        "expected file {} to {expected}",
        path.display()
    );
    Ok(())
}

fn create_directory_in_workspace(temp_path: &Path, name: &DirectoryName) -> Result<()> {
    let dir_path = temp_path.join(name.as_str());
    fs::create_dir_all(&dir_path)
        .with_context(|| format!("create directory {}", dir_path.display()))?;
    Ok(())
}

/// Result from running the netsuke manifest command.
struct RunResult {
    stdout: String,
    stderr: String,
    success: bool,
}

fn run_manifest_command(temp_path: &Path, output: &ManifestOutputPath) -> Result<RunResult> {
    let args = ["manifest", output.as_str()];
    let run = run_netsuke_in(temp_path, &args)?;
    Ok(RunResult {
        stdout: run.stdout,
        stderr: run.stderr,
        success: run.success,
    })
}

fn store_run_result(world: &TestWorld, result: RunResult) {
    // Store raw command outputs first
    world.command_stdout.set(result.stdout);
    world.command_stderr.set(result.stderr.clone());

    // Then record success/failure status with error message
    world.run_status.set(result.success);
    if result.success {
        world.run_error.clear();
    } else {
        world.run_error.set(result.stderr);
    }
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a minimal Netsuke workspace")]
fn minimal_workspace(world: &TestWorld) -> Result<()> {
    let temp = tempfile::tempdir().context("create temp dir for manifest workspace")?;
    let netsukefile = temp.path().join("Netsukefile");
    fs::copy("tests/data/minimal.yml", &netsukefile)
        .with_context(|| format!("copy manifest to {}", netsukefile.display()))?;
    *world.temp_dir.borrow_mut() = Some(temp);
    world.run_status.clear();
    world.run_error.clear();
    world.command_stdout.clear();
    world.command_stderr.clear();
    Ok(())
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[given("a directory named {name:string} exists")]
fn directory_named_exists(world: &TestWorld, name: &str) -> Result<()> {
    let name = DirectoryName::new(name);
    let temp_path = get_temp_path(world)?;
    create_directory_in_workspace(&temp_path, &name)
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("the netsuke manifest subcommand is run with {output:string}")]
fn run_manifest_subcommand(world: &TestWorld, output: &str) -> Result<()> {
    let output = ManifestOutputPath::new(output);
    let temp_path = get_temp_path(world)?;
    let result = run_manifest_command(&temp_path, &output)?;
    store_run_result(world, result);
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[then("stdout should contain {fragment:string}")]
fn stdout_should_contain(world: &TestWorld, fragment: &str) -> Result<()> {
    let fragment = OutputFragment::new(fragment);
    assert_output_contains(&world.command_stdout, OutputType::Stdout, &fragment)
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[then("stderr should contain {fragment:string}")]
fn stderr_should_contain(world: &TestWorld, fragment: &str) -> Result<()> {
    let fragment = OutputFragment::new(fragment);
    assert_output_contains(&world.command_stderr, OutputType::Stderr, &fragment)
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[then("the file {name:string} should exist")]
fn file_should_exist(world: &TestWorld, name: &str) -> Result<()> {
    let name = FileName::new(name);
    assert_file_existence(world, &name, true)
}

#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[then("the file {name:string} should not exist")]
fn file_should_not_exist(world: &TestWorld, name: &str) -> Result<()> {
    let name = FileName::new(name);
    assert_file_existence(world, &name, false)
}

// ---------------------------------------------------------------------------
// Missing manifest scenario steps
// ---------------------------------------------------------------------------

/// Create an empty workspace (no Netsukefile).
#[given("an empty workspace")]
fn empty_workspace(world: &TestWorld) -> Result<()> {
    let temp = tempfile::tempdir().context("create temp dir for empty workspace")?;
    *world.temp_dir.borrow_mut() = Some(temp);
    world.run_status.clear();
    world.run_error.clear();
    world.command_stdout.clear();
    world.command_stderr.clear();
    Ok(())
}

/// Create an empty workspace at a specific path.
///
/// This step sets up a fixed-path workspace for scenarios that test the `-C`
/// flag by creating the directory at the specified path and storing a tempdir
/// in the world so subsequent steps can access it.
#[given("an empty workspace at path {path:string}")]
fn empty_workspace_at_path(world: &TestWorld, path: &str) -> Result<()> {
    // Ensure the directory exists and is empty.
    let dir = Path::new(path);
    if dir.exists() {
        fs::remove_dir_all(dir).with_context(|| format!("remove existing {}", dir.display()))?;
    }
    fs::create_dir_all(dir).with_context(|| format!("create directory {}", dir.display()))?;
    // Use a normal temp dir as the working directory for the netsuke command.
    // The -C flag in the arguments will override where netsuke looks for files.
    let temp = tempfile::tempdir().context("create temp dir for command execution")?;
    *world.temp_dir.borrow_mut() = Some(temp);
    // Clear world state for consistency.
    world.run_status.clear();
    world.run_error.clear();
    world.command_stdout.clear();
    world.command_stderr.clear();
    Ok(())
}

/// Run netsuke without any arguments.
#[when("netsuke is run without arguments")]
fn run_netsuke_no_args(world: &TestWorld) -> Result<()> {
    let temp_path = get_temp_path(world)?;
    let args: [&str; 0] = [];
    let run = run_netsuke_in(&temp_path, &args)?;
    store_run_result(
        world,
        RunResult {
            stdout: run.stdout,
            stderr: run.stderr,
            success: run.success,
        },
    );
    Ok(())
}

/// Run netsuke with specified arguments.
#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("netsuke is run with arguments {args:string}")]
fn run_netsuke_with_args(world: &TestWorld, args: &str) -> Result<()> {
    let temp_path = get_temp_path(world)?;
    let args: Vec<&str> = args.split_whitespace().collect();
    let run = run_netsuke_in(&temp_path, &args)?;
    store_run_result(
        world,
        RunResult {
            stdout: run.stdout,
            stderr: run.stderr,
            success: run.success,
        },
    );
    Ok(())
}
