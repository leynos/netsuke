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
    if world.original_path.is_none() {
        world.original_path = Some(std::env::var_os("PATH").unwrap_or_default());
    }
    let dir_path = dir.path().display().to_string();
    let new_path = if let Some(old) = world.original_path.as_ref() {
        format!("{dir_path}:{}", old.to_string_lossy())
    } else {
        dir_path
    };
    unsafe {
        std::env::set_var("PATH", new_path);
    }
    world.temp = Some(dir);
}

#[given("no ninja executable is available")]
fn no_ninja(world: &mut CliWorld) {
    let dir = tempfile::tempdir().expect("temp dir");
    if world.original_path.is_none() {
        world.original_path = Some(std::env::var_os("PATH").unwrap_or_default());
    }
    unsafe {
        std::env::set_var("PATH", dir.path());
    }
    world.temp = Some(dir);
}

#[when("the ninja process is run")]
fn run(world: &mut CliWorld) {
    let cli = world.cli.as_ref().expect("cli");
    match runner::run(cli) {
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
}
