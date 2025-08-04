//! Step definitions for Ninja process execution.

use crate::{CliWorld, support};
use cucumber::{given, then, when};
use netsuke::runner;

/// Creates a fake ninja executable that exits with the given status code.
#[given(expr = "a fake ninja executable that exits with {int}")]
fn fake_ninja(world: &mut CliWorld, code: i32) {
    let (dir, path) = support::fake_ninja(code);
    world.ninja = Some(path.to_string_lossy().into_owned());
    world.temp = Some(dir);
}

/// Sets up a scenario where no ninja executable is available.
///
/// This step creates a temporary directory and sets the ninja path to a
/// non-existent executable within that directory, allowing tests to verify
/// behaviour when ninja is not found on the system.
#[given("no ninja executable is available")]
fn no_ninja(world: &mut CliWorld) {
    let dir = tempfile::tempdir().expect("temp dir");
    world.ninja = Some(dir.path().join("ninja").to_string_lossy().into_owned());
    world.temp = Some(dir);
}

/// Executes the ninja process and captures the result in the test world.
///
/// This step runs the ninja executable (either real or fake) using the CLI
/// configuration stored in the world, then updates the world's `run_status` and
/// `run_error` fields based on the execution outcome.
#[when("the ninja process is run")]
fn run(world: &mut CliWorld) {
    let cli = world.cli.as_ref().expect("cli");
    let program = world
        .ninja
        .as_ref()
        .map_or_else(|| std::path::Path::new("ninja"), std::path::Path::new);
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
