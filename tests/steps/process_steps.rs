//! Step definitions for Ninja process execution.

use crate::{CliWorld, support};
use cucumber::{given, then, when};
use netsuke::runner;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

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

/// Creates a fake ninja executable that validates the build file path.
#[given("a fake ninja executable that checks for the build file")]
fn fake_ninja_check(world: &mut CliWorld) {
    let (dir, path) = support::fake_ninja_check_build_file();
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

/// Updates the CLI to use the temporary directory created for the fake ninja.
#[given("the CLI uses the temporary directory")]
fn cli_uses_temp_dir(world: &mut CliWorld) {
    let temp = world.temp.as_ref().expect("temp dir");
    let cli = world.cli.as_mut().expect("cli");
    cli.directory = Some(temp.path().to_path_buf());
}

/// Creates a directory named `build.ninja` in the temporary working directory.
#[given("a directory named build.ninja exists")]
fn build_dir_exists(world: &mut CliWorld) {
    let temp = world.temp.as_ref().expect("temp dir");
    fs::create_dir(temp.path().join("build.ninja")).expect("create dir");
}

/// Executes the ninja process and captures the result in the test world.
///
/// This step runs the `ninja` executable using the CLI configuration stored in
/// the world, then updates the world's `run_status` and `run_error` fields based
/// on the execution outcome.
#[expect(
    clippy::option_if_let_else,
    reason = "explicit conditional is clearer than map_or_else"
)]
#[when("the ninja process is run")]
fn run(world: &mut CliWorld) {
    let cli = world.cli.as_ref().expect("cli");
    let program = if let Some(ninja) = &world.ninja {
        std::path::Path::new(ninja)
    } else {
        std::path::Path::new("ninja")
    };
    match runner::run_ninja(program, cli, Path::new("build.ninja"), &[]) {
        Ok(()) => {
            world.run_status = Some(true);
            world.run_error = None;
        }
        Err(e) => {
            world.run_status = Some(false);
            world.run_error = Some(e.to_string());
        }
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
