//! Step definitions for `netsuke manifest` behavioural tests.

use crate::bdd::fixtures::with_world;
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

fn get_temp_path() -> Result<PathBuf> {
    with_world(|world| {
        let temp = world.temp_dir.borrow();
        let dir = temp.as_ref().context("temp dir has not been initialised")?;
        Ok(dir.path().to_path_buf())
    })
}

fn assert_output_contains(
    output: &Slot<String>,
    output_type: OutputType,
    fragment: &OutputFragment,
) -> Result<()> {
    let content = output
        .get()
        .with_context(|| format!("no {output_type} captured from netsuke CLI process"))?;
    ensure!(
        content.contains(fragment.as_str()),
        "expected {output_type} to contain '{}', got '{content}'",
        fragment.as_str()
    );
    Ok(())
}

fn resolve_file_path(temp_path: &Path, name: &FileName) -> PathBuf {
    temp_path.join(name.as_str())
}

fn check_file_exists(path: &Path) -> bool {
    path.exists()
}

fn assert_file_existence(name: &FileName, should_exist: bool) -> Result<()> {
    let temp_path = get_temp_path()?;
    let path = resolve_file_path(&temp_path, name);
    let expected = if should_exist { "exist" } else { "not exist" };
    ensure!(
        check_file_exists(&path) == should_exist,
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

fn store_run_result(result: RunResult) {
    with_world(|world| {
        world.command_stdout.set(result.stdout);
        world.command_stderr.set(result.stderr);
        world.run_status.set(result.success);
        if result.success {
            world.run_error.clear();
        } else if let Some(stderr) = world.command_stderr.get() {
            world.run_error.set(stderr);
        }
    });
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a minimal Netsuke workspace")]
fn minimal_workspace() -> Result<()> {
    let temp = tempfile::tempdir().context("create temp dir for manifest workspace")?;
    let netsukefile = temp.path().join("Netsukefile");
    fs::copy("tests/data/minimal.yml", &netsukefile)
        .with_context(|| format!("copy manifest to {}", netsukefile.display()))?;
    with_world(|world| {
        *world.temp_dir.borrow_mut() = Some(temp);
        world.run_status.clear();
        world.run_error.clear();
        world.command_stdout.clear();
        world.command_stderr.clear();
    });
    Ok(())
}

#[given("a directory named {name} exists")]
fn directory_named_exists(name: String) -> Result<()> {
    let name = DirectoryName::new(name);
    let temp_path = get_temp_path()?;
    create_directory_in_workspace(&temp_path, &name)
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the netsuke manifest subcommand is run with {output}")]
fn run_manifest_subcommand(output: String) -> Result<()> {
    let output = ManifestOutputPath::new(output);
    let temp_path = get_temp_path()?;
    let result = run_manifest_command(&temp_path, &output)?;
    store_run_result(result);
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("stdout should contain {fragment}")]
fn stdout_should_contain(fragment: String) -> Result<()> {
    let fragment = OutputFragment::new(fragment);
    with_world(|world| assert_output_contains(&world.command_stdout, OutputType::Stdout, &fragment))
}

#[then("stderr should contain {fragment}")]
fn stderr_should_contain(fragment: String) -> Result<()> {
    let fragment = OutputFragment::new(fragment);
    with_world(|world| assert_output_contains(&world.command_stderr, OutputType::Stderr, &fragment))
}

#[then("the file {name} should exist")]
fn file_should_exist(name: String) -> Result<()> {
    let name = FileName::new(name);
    assert_file_existence(&name, true)
}

#[then("the file {name} should not exist")]
fn file_should_not_exist(name: String) -> Result<()> {
    let name = FileName::new(name);
    assert_file_existence(&name, false)
}
