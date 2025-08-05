//! Step definitions for Ninja process execution.

use crate::{CliWorld, support};
use cucumber::{given, then, when};
use netsuke::runner;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{NamedTempFile, TempDir};

/// Installs a test-specific ninja binary and updates the `PATH`.
#[expect(
    clippy::needless_pass_by_value,
    reason = "helper owns path for simplicity"
)]
fn install_test_ninja(world: &mut CliWorld, dir: TempDir, ninja_path: PathBuf) {
    let original = world
        .original_path
        .get_or_insert_with(|| std::env::var_os("PATH").unwrap_or_default());

    let new_path = format!("{}:{}", dir.path().display(), original.to_string_lossy());
    // SAFETY: nightly marks `set_var` as unsafe; override path for test isolation.
    unsafe {
        std::env::set_var("PATH", &new_path);
    }

    world.ninja = Some(ninja_path.to_string_lossy().into_owned());
    world.temp = Some(dir);
}

/// Creates a fake ninja executable that exits with the given status code.
#[given(expr = "a fake ninja executable that exits with {int}")]
fn fake_ninja(world: &mut CliWorld, code: i32) {
    let (dir, path) = support::fake_ninja(code);
    install_test_ninja(world, dir, path);
}

/// Sets up a scenario where no ninja executable is available.
///
/// This step creates a temporary directory and records the path to a
/// non-existent `ninja` binary within that directory, allowing tests to verify
/// behaviour when the executable is missing.
#[given("no ninja executable is available")]
fn no_ninja(world: &mut CliWorld) {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("ninja");
    install_test_ninja(world, dir, path);
}

/// Executes the ninja process and captures the result in the test world.
///
/// This step runs the `ninja` executable using the CLI configuration stored in
/// the world, then updates the world's `run_status` and `run_error` fields based
/// on the execution outcome.
#[when("the ninja process is run")]
fn run(world: &mut CliWorld) {
    // Touch the capture variant so the support module's helpers remain used.
    let _ = support::fake_ninja_capture as fn() -> (TempDir, PathBuf, PathBuf);
    let cli = world.cli.as_mut().expect("cli");

    // Ensure a manifest exists at the path expected by the CLI.
    let dir = world.temp.as_ref().expect("temp dir");
    let manifest_path = if cli.file.is_absolute() {
        cli.file.clone()
    } else {
        dir.path().join(&cli.file)
    };
    if !manifest_path.exists() {
        let mut file = NamedTempFile::new_in(dir.path()).expect("manifest");
        support::write_manifest(&mut file);
        // Persist the temporary file to the desired manifest path.
        file.persist(&manifest_path).expect("persist manifest");
    }
    cli.file.clone_from(&manifest_path);

    let program = world
        .ninja
        .as_ref()
        .map_or_else(|| Path::new("ninja"), Path::new);

    match runner::run_ninja(program, cli, &[]) {
        Ok(()) => {
            world.run_status = Some(true);
            world.run_error = None;
        }
        Err(e) => {
            world.run_status = Some(false);
            world.run_error = Some(e.to_string());
        }
    }

    // Clean up any manifest left outside the temporary directory.
    if !manifest_path.starts_with(dir.path()) {
        let _ = fs::remove_file(manifest_path);
    }
}

/// Asserts that the command succeeds.
#[then("the command should succeed")]
fn command_should_succeed(world: &mut CliWorld) {
    assert_eq!(world.run_status, Some(true));
}

/// Asserts that the command fails and records an error message.
#[then("the command should fail")]
fn command_should_fail(world: &mut CliWorld) {
    assert_eq!(world.run_status, Some(false));
    assert!(
        world.run_error.is_some(),
        "Expected an error message, but none was found",
    );
}

/// Asserts that the command failed and the error message matches the expected value.
#[expect(
    clippy::needless_pass_by_value,
    reason = "cucumber step parameters require owned Strings"
)]
#[then(expr = "the command should fail with error {string}")]
fn command_should_fail_with_error(world: &mut CliWorld, expected: String) {
    assert_eq!(world.run_status, Some(false));
    let actual = world
        .run_error
        .as_ref()
        .expect("Expected an error message, but none was found");
    assert!(
        actual.contains(&expected),
        "Expected error message to contain '{expected}', but got '{actual}'",
    );
}
