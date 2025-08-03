//! Step definitions for Ninja process execution.

use crate::CliWorld;
use cucumber::{given, then, when};
use netsuke::runner;
use std::fs::{self, File};
use std::io::Write;

#[given(expr = "a fake ninja executable that exits with {int}")]
fn fake_ninja(world: &mut CliWorld, code: i32) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("ninja");
    let mut file = File::create(&path).expect("script");
    writeln!(file, "#!/bin/sh\nexit {code}").expect("write script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("perms");
    }
    world.ninja = Some(path.to_string_lossy().into_owned());
    world.temp = Some(dir);
}

#[given("no ninja executable is available")]
fn no_ninja(world: &mut CliWorld) {
    let dir = tempfile::tempdir().expect("temp dir");
    world.ninja = Some(dir.path().join("ninja").to_string_lossy().into_owned());
    world.temp = Some(dir);
}

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

#[then("the command should succeed")]
fn command_should_succeed(world: &mut CliWorld) {
    assert_eq!(world.run_status, Some(true));
}

#[then("the command should fail")]
fn command_should_fail(world: &mut CliWorld) {
    assert_eq!(world.run_status, Some(false));
    assert!(
        world.run_error.is_some(),
        "Expected an error message, but none was found"
    );
}

/// Asserts that the command failed and the error message matches the expected value.
#[allow(
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
        "Expected error message to contain '{expected}', but got '{actual}'"
    );
}
