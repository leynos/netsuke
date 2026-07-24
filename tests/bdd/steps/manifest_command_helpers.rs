//! Shared helpers for the `netsuke generate` BDD step definitions.
//!
//! Split from `manifest_command.rs` so both files stay within the module size
//! budget; the step definitions in the parent module call these helpers.

use super::OutputType;
use crate::bdd::fixtures::TestWorld;
use crate::bdd::helpers::assertions::{assert_slot_contains, normalize_fluent_isolates};
use crate::bdd::types::{DirectoryName, FileName, ManifestOutputPath, OutputFragment};
use anyhow::{Context, Result, ensure};
use rstest_bdd::Slot;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn get_temp_path(world: &TestWorld) -> Result<PathBuf> {
    let temp = world.temp_dir.borrow();
    let dir = temp.as_ref().context("temp dir has not been initialised")?;
    Ok(dir.path().to_path_buf())
}

pub(super) fn assert_output_contains(
    output: &Slot<String>,
    output_type: OutputType,
    fragment: &OutputFragment,
) -> Result<()> {
    assert_slot_contains(output, fragment.as_str(), &output_type.to_string())
}

pub(super) fn assert_output_not_contains(
    output: &Slot<String>,
    output_type: OutputType,
    fragment: &OutputFragment,
) -> Result<()> {
    let value = output
        .get()
        .with_context(|| format!("{output_type} output should be captured"))?;
    let normalized_value = normalize_fluent_isolates(&value);
    let normalized_fragment = normalize_fluent_isolates(fragment.as_str());
    ensure!(
        !normalized_value.contains(&normalized_fragment),
        "expected {output_type} to omit '{fragment}', but it was present in:\n{value}",
    );
    Ok(())
}

pub(super) fn assert_file_existence(
    world: &TestWorld,
    name: &FileName,
    should_exist: bool,
) -> Result<()> {
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

pub(super) fn create_directory_in_workspace(temp_path: &Path, name: &DirectoryName) -> Result<()> {
    let dir_path = temp_path.join(name.as_str());
    fs::create_dir_all(&dir_path)
        .with_context(|| format!("create directory {}", dir_path.display()))?;
    Ok(())
}

/// Result from running the netsuke generate command.
pub(super) struct RunResult {
    stdout: String,
    stderr: String,
    success: bool,
}

impl RunResult {
    /// Build a `RunResult` from a completed process's captured output, so the
    /// stdout/stderr decoding and status handling stay identical across callers.
    pub(super) fn from_output(output: std::process::Output) -> Self {
        let std::process::Output {
            status,
            stdout,
            stderr,
        } = output;
        Self {
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
            success: status.success(),
        }
    }
}

pub(super) fn run_generate_command(
    world: &TestWorld,
    output: &ManifestOutputPath,
) -> Result<RunResult> {
    let args = if output.as_str() == "-" {
        vec!["generate"]
    } else {
        vec!["generate", "--output", output.as_str()]
    };
    let mut cmd = build_netsuke_command(world, &args)?;
    let result = cmd.output().context("run netsuke generate command")?;
    Ok(RunResult::from_output(result))
}

pub(super) fn store_run_result(world: &TestWorld, result: RunResult) {
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

/// Locate the netsuke executable using `assert_cmd`'s binary locator.
pub(super) fn netsuke_executable() -> Result<PathBuf> {
    let exe = assert_cmd::cargo::cargo_bin!("netsuke");
    ensure!(
        exe.is_file(),
        "netsuke binary not found at {}",
        exe.display()
    );
    Ok(exe.to_path_buf())
}

/// Build a netsuke command with a sanitized environment.
///
/// This helper constructs an `assert_cmd::Command` configured with:
/// - The resolved netsuke executable path
/// - Current directory set to the test workspace
/// - Cleared inherited environment to ensure test isolation
/// - Only scenario-specific env vars (from BDD steps) are preserved
/// - Controlled `PATH` variable
///
/// Returns the configured command ready for execution.
pub(super) fn build_netsuke_command(
    world: &TestWorld,
    args: &[&str],
) -> Result<assert_cmd::Command> {
    let temp_path = get_temp_path(world)?;

    let mut cmd = assert_cmd::Command::new(netsuke_executable()?);
    cmd.current_dir(&temp_path).env_clear().args(args);

    // Read PATH without holding EnvLock.
    //
    // Two cases apply:
    // 1. A NinjaEnvGuard is alive in world.ninja_env_guard — that guard holds
    //    EnvLock for the scenario lifetime, so no concurrent thread can mutate
    //    any env var; the read is therefore safe.
    // 2. No NinjaEnvGuard is alive — PATH is mutated only inside
    //    prepend_dir_to_path, which holds EnvLock only for the duration of the
    //    set_var call.  That mutation completes before build_netsuke_command is
    //    called, so the read is safe.
    //
    // Acquiring EnvLock here would deadlock when case 1 applies because Mutex
    // is not reentrant and the same thread already holds the lock via
    // NinjaEnvGuard.
    if let Some(host_path) = std::env::var_os("PATH") {
        cmd.env("PATH", host_path);
    }

    // Forward scenario-tracked vars from TestWorld state (never reads process env).
    let env_vars_forward = world.env_vars_forward.borrow();
    for (key, value) in env_vars_forward.iter() {
        cmd.env(key, value);
    }

    Ok(cmd)
}

/// Run netsuke with the given arguments and store the result.
pub(super) fn run_netsuke_and_store(world: &TestWorld, args: &[&str]) -> Result<()> {
    let mut cmd = build_netsuke_command(world, args)?;

    let output = cmd.output().context("run netsuke command")?;

    store_run_result(world, RunResult::from_output(output));
    Ok(())
}
