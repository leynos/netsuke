//! Step definitions for `netsuke manifest` behavioural tests.

use crate::bdd::fixtures::TestWorld;
use crate::bdd::helpers::assertions::{assert_slot_contains, normalize_fluent_isolates};
use crate::bdd::types::{
    CliArgs, DirectoryName, FileName, ManifestOutputPath, OutputFragment, PathString,
};
use anyhow::{Context, Result, ensure};
use rstest_bdd::Slot;
use rstest_bdd_macros::{given, then, when};
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

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

fn assert_output_not_contains(
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

fn run_manifest_command(world: &TestWorld, output: &ManifestOutputPath) -> Result<RunResult> {
    let args = ["manifest", output.as_str()];
    let mut cmd = build_netsuke_command(world, &args)?;
    let result = cmd.output().context("run netsuke manifest command")?;
    Ok(RunResult {
        stdout: String::from_utf8_lossy(&result.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&result.stderr).into_owned(),
        success: result.status.success(),
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

/// Locate the netsuke executable using `assert_cmd`'s binary locator.
fn netsuke_executable() -> Result<PathBuf> {
    let exe = assert_cmd::cargo::cargo_bin!("netsuke");
    ensure!(
        exe.is_file(),
        "netsuke binary not found at {}",
        exe.display()
    );
    Ok(exe.to_path_buf())
}

/// Build a netsuke command with a sanitised environment.
///
/// This helper constructs an `assert_cmd::Command` configured with:
/// - The resolved netsuke executable path
/// - Current directory set to the test workspace
/// - Cleared inherited environment to ensure test isolation
/// - Only scenario-specific env vars (from BDD steps) are preserved
/// - Controlled `PATH` variable
///
/// Returns the configured command ready for execution.
fn build_netsuke_command(world: &TestWorld, args: &[&str]) -> Result<assert_cmd::Command> {
    let temp_path = get_temp_path(world)?;

    let mut cmd = assert_cmd::Command::new(netsuke_executable()?);
    cmd.current_dir(&temp_path).env_clear().args(args);

    // Forward host PATH so netsuke can exec ninja.
    // Acquire EnvLock only for this one read so it is consistent with
    // any concurrent VarGuard mutations. NinjaEnvGuard is NOT held at
    // this point because NETSUKE_NINJA is now tracked via env_vars_forward,
    // so there is no re-entrant deadlock risk.
    {
        let _lock = test_support::env_lock::EnvLock::acquire();
        if let Some(host_path) = std::env::var_os("PATH") {
            cmd.env("PATH", host_path);
        }
    }

    // Forward scenario-tracked vars from TestWorld state (never reads process env).
    let env_vars_forward = world.env_vars_forward.borrow();
    for (key, value) in env_vars_forward.iter() {
        cmd.env(key, value);
    }

    Ok(cmd)
}

/// Run netsuke with the given arguments and store the result.
fn run_netsuke_and_store(world: &TestWorld, args: &[&str]) -> Result<()> {
    let mut cmd = build_netsuke_command(world, args)?;

    let output = cmd.output().context("run netsuke command")?;

    store_run_result(
        world,
        RunResult {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            success: output.status.success(),
        },
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a minimal Netsuke workspace")]
fn minimal_workspace(world: &TestWorld) -> Result<()> {
    let temp = tempfile::tempdir().context("create temp dir for manifest workspace")?;
    let netsukefile = temp.path().join("Netsukefile");
    // Use absolute path to avoid issues when CWD is changed by parallel tests
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let minimal_yml_path = std::path::Path::new(manifest_dir).join("tests/data/minimal.yml");
    fs::copy(&minimal_yml_path, &netsukefile)
        .with_context(|| format!("copy manifest to {}", netsukefile.display()))?;
    *world.temp_dir.borrow_mut() = Some(temp);
    world.run_status.clear();
    world.run_error.clear();
    world.command_stdout.clear();
    world.command_stderr.clear();
    Ok(())
}

#[given("a directory named {name:string} exists")]
fn directory_named_exists(world: &TestWorld, name: DirectoryName) -> Result<()> {
    let temp_path = get_temp_path(world)?;
    create_directory_in_workspace(&temp_path, &name)
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the netsuke manifest subcommand is run with {output:string}")]
fn run_manifest_subcommand(world: &TestWorld, output: ManifestOutputPath) -> Result<()> {
    let result = run_manifest_command(world, &output)?;
    store_run_result(world, result);
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("stdout should contain {fragment:string}")]
fn stdout_should_contain(world: &TestWorld, fragment: OutputFragment) -> Result<()> {
    assert_output_contains(&world.command_stdout, OutputType::Stdout, &fragment)
}

#[then("stderr should contain {fragment:string}")]
fn stderr_should_contain(world: &TestWorld, fragment: OutputFragment) -> Result<()> {
    assert_output_contains(&world.command_stderr, OutputType::Stderr, &fragment)
}

#[then("stdout should not contain {fragment:string}")]
fn stdout_should_not_contain(world: &TestWorld, fragment: OutputFragment) -> Result<()> {
    assert_output_not_contains(&world.command_stdout, OutputType::Stdout, &fragment)
}

#[then("stderr should not contain {fragment:string}")]
fn stderr_should_not_contain(world: &TestWorld, fragment: OutputFragment) -> Result<()> {
    assert_output_not_contains(&world.command_stderr, OutputType::Stderr, &fragment)
}

fn assert_output_ordering(
    output: &Slot<String>,
    output_type: OutputType,
    first: &OutputFragment,
    second: &OutputFragment,
) -> Result<()> {
    let value = output
        .get()
        .with_context(|| format!("{output_type} output should be captured"))?;
    let normalized_value = normalize_fluent_isolates(&value);
    let normalized_first = normalize_fluent_isolates(first.as_str());
    let normalized_second = normalize_fluent_isolates(second.as_str());
    let first_pos = normalized_value
        .find(&normalized_first)
        .with_context(|| format!("{output_type} should contain '{first}'"))?;
    // Search for second only after the end of the first match to handle
    // repeated fragments correctly (e.g., "A" before "A" in "A A").
    let search_start = first_pos + normalized_first.len();
    let remainder = normalized_value
        .get(search_start..)
        .with_context(|| "search_start should be a valid UTF-8 boundary")?;
    let second_pos = remainder
        .find(&normalized_second)
        .with_context(|| format!("{output_type} should contain '{second}' after '{first}'"))?;
    // Convert relative position to absolute position for error reporting
    let absolute_second_pos = search_start + second_pos;
    ensure!(
        first_pos < absolute_second_pos,
        concat!(
            "expected '{first}' to appear before '{second}' in {output_type}, ",
            "but positions were {first_pos} and {absolute_second_pos}"
        ),
    );
    Ok(())
}

#[then("stdout should contain {first:string} before {second:string}")]
fn stdout_should_contain_in_order(
    world: &TestWorld,
    first: OutputFragment,
    second: OutputFragment,
) -> Result<()> {
    assert_output_ordering(&world.command_stdout, OutputType::Stdout, &first, &second)
}

#[then("the file {name:string} should exist")]
fn file_should_exist(world: &TestWorld, name: FileName) -> Result<()> {
    assert_file_existence(world, &name, true)
}

#[then("the file {name:string} should not exist")]
fn file_should_not_exist(world: &TestWorld, name: FileName) -> Result<()> {
    assert_file_existence(world, &name, false)
}

// ---------------------------------------------------------------------------
// Missing manifest scenario steps
// ---------------------------------------------------------------------------

/// Create an empty workspace (no Netsukefile).
#[given("an empty workspace")]
fn empty_workspace(world: &TestWorld) -> Result<()> {
    let temp = tempfile::tempdir().context("create temp dir for empty workspace")?;
    // Store the workspace path for use by run_netsuke_with_directory_flag
    *world.workspace_path.borrow_mut() = Some(temp.path().to_path_buf());
    *world.temp_dir.borrow_mut() = Some(temp);
    world.run_status.clear();
    world.run_error.clear();
    world.command_stdout.clear();
    world.command_stderr.clear();
    Ok(())
}

/// Normalize a path by resolving `.` and `..` components without requiring the
/// path to exist (unlike `std::fs::canonicalize`).
fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                normalized.pop();
            }
            Component::CurDir => {}
            c => normalized.push(c),
        }
    }
    normalized
}

/// Resolve a path as fully as possible, canonicalizing existing ancestors and
/// normalizing the remaining components. This handles symlinks in existing
/// parts of the path while still allowing the final target to not yet exist.
fn resolve_path_safe(path: &Path) -> Result<PathBuf> {
    // First normalize the path to remove . and .. components
    let normalized = normalize_path(path);

    // Find the longest existing ancestor we can canonicalize
    let mut existing_ancestor = normalized.clone();
    let mut remaining_components = Vec::new();

    while !existing_ancestor.as_os_str().is_empty() && !existing_ancestor.exists() {
        if let Some(file_name) = existing_ancestor.file_name() {
            remaining_components.push(file_name.to_owned());
        }
        if !existing_ancestor.pop() {
            break;
        }
    }

    // Canonicalize the existing ancestor to resolve any symlinks
    let resolved_base = if existing_ancestor.exists() {
        fs::canonicalize(&existing_ancestor)
            .with_context(|| format!("canonicalize {}", existing_ancestor.display()))?
    } else {
        // No existing ancestor found, use the normalized path as-is
        normalized.clone()
    };

    // Append the remaining components that didn't exist
    let mut resolved = resolved_base;
    for component in remaining_components.into_iter().rev() {
        resolved.push(component);
    }

    Ok(resolved)
}

/// Create an empty workspace at a specific path.
///
/// This step sets up a fixed-path workspace for scenarios that test the `-C`
/// flag by creating the directory at the specified path and storing a tempdir
/// in the world so subsequent steps can access it.
///
/// # Errors
///
/// Returns an error if the path is outside expected test locations (must be a
/// subdirectory of `/tmp` or the system temp directory, not the root itself)
/// to prevent accidental deletion of sensitive directories.
#[given("an empty workspace at path {path:string}")]
fn empty_workspace_at_path(world: &TestWorld, path: PathString) -> Result<()> {
    let dir = path.as_path();
    // Resolve the path by canonicalizing existing ancestors and normalizing the
    // rest. This prevents symlink-based traversal attacks like creating a
    // symlink `/tmp/escape -> /` and then using `/tmp/escape/etc/passwd`.
    let resolved = resolve_path_safe(dir)?;

    // Canonicalize the system temp directory for accurate comparison
    let temp_dir_raw = std::env::temp_dir();
    let temp_dir = fs::canonicalize(&temp_dir_raw).unwrap_or(temp_dir_raw);

    // Also canonicalize /tmp if it exists (it may be a symlink on some systems)
    let tmp_path = Path::new("/tmp");
    let canonical_tmp = fs::canonicalize(tmp_path).unwrap_or_else(|_| tmp_path.to_path_buf());

    // Safeguard: only allow paths that are proper subdirectories of /tmp or
    // the system temp directory (not the root temp directory itself).
    let is_safe_tmp = resolved.starts_with(&canonical_tmp) && resolved != canonical_tmp;
    let is_safe_temp = resolved.starts_with(&temp_dir) && resolved != temp_dir;
    ensure!(
        is_safe_tmp || is_safe_temp,
        "test workspace path must be a subdirectory of /tmp or system temp directory, not the root itself: {}",
        resolved.display()
    );
    // Ensure the directory exists and is empty.
    if resolved.exists() {
        fs::remove_dir_all(&resolved)
            .with_context(|| format!("remove existing {}", resolved.display()))?;
    }
    fs::create_dir_all(&resolved)
        .with_context(|| format!("create directory {}", resolved.display()))?;

    // Store the workspace path for use by run_netsuke_with_directory_flag
    *world.workspace_path.borrow_mut() = Some(resolved);
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
    run_netsuke_and_store(world, &[])
}

/// Run netsuke with specified arguments.
///
/// # Limitations
///
/// Arguments are split on whitespace using `split_whitespace()`, which does not
/// handle quoted arguments containing spaces. For example, `-f "my file.yml"`
/// would be incorrectly split into `["-f", "\"my", "file.yml\""]`. Current test
/// scenarios use only simple arguments without embedded spaces.
#[expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd macro generates wrapper; FIXME: https://github.com/leynos/rstest-bdd/issues/381"
)]
#[when("netsuke is run with arguments {args:string}")]
fn run_netsuke_with_args(world: &TestWorld, args: CliArgs) -> Result<()> {
    let args: Vec<&str> = args.as_str().split_whitespace().collect();
    run_netsuke_and_store(world, &args)
}

/// Run netsuke with `-C` pointing to the workspace directory.
///
/// This step runs netsuke with the `-C` flag set to the workspace path created
/// by `empty_workspace_at_path`, allowing tests to verify the directory flag
/// behaviour without hardcoded paths.
#[when("netsuke is run with directory flag pointing to the workspace")]
fn run_netsuke_with_directory_flag(world: &TestWorld) -> Result<()> {
    let workspace_path = world
        .workspace_path
        .borrow()
        .clone()
        .context("workspace_path must be set by empty_workspace_at_path step")?;
    let dir_arg = workspace_path.to_string_lossy().to_string();
    run_netsuke_and_store(world, &["-C", &dir_arg])
}

// ---------------------------------------------------------------------------
// Unit tests for environment-handling behaviour
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::fixture;
    use std::ffi::OsStr;
    use test_support::env::VarGuard;

    fn env_value<'a>(cmd: &'a assert_cmd::Command, key: &str) -> Option<&'a OsStr> {
        cmd.get_envs()
            .find(|(k, _)| *k == OsStr::new(key))
            .and_then(|(_, v)| v)
    }

    #[fixture]
    fn prepared_world() -> TestWorld {
        let world = TestWorld::default();
        let temp = tempfile::tempdir().expect("create temp dir");
        *world.temp_dir.borrow_mut() = Some(temp);
        world
    }

    #[rstest::rstest]
    fn world_env_vars_with_value_are_applied(prepared_world: TestWorld) {
        let world = prepared_world;

        // Track the env var in TestWorld's forward map - this is the value that
        // will be forwarded to the child command, not read from process env.
        world.track_env_var(
            "NETSUKE_TEST_FLAG".to_owned(),
            None,
            Some(OsString::from("enabled")),
        );

        let cmd = build_netsuke_command(&world, &["--help"]).expect("build command");

        let val =
            env_value(&cmd, "NETSUKE_TEST_FLAG").expect("NETSUKE_TEST_FLAG should be present");
        assert_eq!(val, OsStr::new("enabled"));
    }

    #[rstest::rstest]
    fn host_env_vars_are_not_inherited(prepared_world: TestWorld) {
        let world = prepared_world;

        // Set a host env var that should NOT be inherited (not tracked in world.env_vars)
        let _guard = VarGuard::set("NETSUKE_HOST_VAR", OsStr::new("should-not-inherit"));

        let cmd = build_netsuke_command(&world, &["--help"]).expect("build command");

        // Command should NOT contain the host env var because env_clear() was called
        let val = env_value(&cmd, "NETSUKE_HOST_VAR");
        assert!(
            val.is_none(),
            "NETSUKE_HOST_VAR should not be inherited from host environment"
        );
    }

    #[rstest::rstest]
    fn host_path_is_forwarded_and_netsuke_executable_is_used(prepared_world: TestWorld) {
        let world = prepared_world;

        // Simulate a different netsuke early in PATH
        let _guard = VarGuard::set("PATH", OsStr::new("/fake/bin"));

        let cmd = build_netsuke_command(&world, &["--version"]).expect("build command");

        // PATH in the command should match what was in the environment when
        // build_netsuke_command was called, forwarded explicitly after env_clear().
        let path_val =
            env_value(&cmd, "PATH").expect("PATH should be explicitly forwarded to the command");
        assert_eq!(path_val, OsStr::new("/fake/bin"));

        // Command should use the resolved netsuke_executable(), not rely on PATH lookup.
        let exe = netsuke_executable().expect("netsuke_executable");
        assert_eq!(cmd.get_program(), exe.as_os_str());
    }
}
