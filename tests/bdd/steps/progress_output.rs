//! Step definitions for progress-output scenarios that require fake Ninja output.

use crate::bdd::fixtures::TestWorld;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use test_support::env::{SystemEnv, override_ninja_env};

fn workspace_root(world: &TestWorld) -> Result<PathBuf> {
    let temp = world.temp_dir.borrow();
    let dir = temp.as_ref().context("temp dir has not been initialised")?;
    Ok(dir.path().to_path_buf())
}

#[cfg(unix)]
fn make_script_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .with_context(|| format!("read metadata for {}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("set executable bit for {}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn make_script_executable(_path: &Path) -> Result<()> {
    Ok(())
}

fn build_fake_ninja_script(lines: &[&str]) -> String {
    if cfg!(windows) {
        let mut script = String::from("@echo off\r\n");
        for line in lines {
            script.push_str("echo ");
            script.push_str(line);
            script.push_str("\r\n");
        }
        script.push_str("exit /B 0\r\n");
        script
    } else {
        let mut script = String::from(
            "#!/bin/sh\nwhile IFS= read -r line; do\n  printf '%s\\n' \"$line\"\ndone <<'NETSUKE_STATUS'\n",
        );
        for line in lines {
            script.push_str(line);
            script.push('\n');
        }
        script.push_str("NETSUKE_STATUS\nexit 0\n");
        script
    }
}

fn fake_ninja_path(root: &Path) -> PathBuf {
    if cfg!(windows) {
        return root.join("fake-ninja-progress.cmd");
    }
    root.join("fake-ninja-progress")
}

fn install_fake_ninja(world: &TestWorld, lines: &[&str]) -> Result<()> {
    let root = workspace_root(world)?;
    let script_path = fake_ninja_path(&root);
    let script = build_fake_ninja_script(lines);
    fs::write(&script_path, script)
        .with_context(|| format!("write fake ninja script {}", script_path.display()))?;
    make_script_executable(&script_path)?;

    let env = SystemEnv::new();
    // Drop any existing guard first so its environment override is restored
    // before installing a replacement for this scenario.
    world.ninja_env_guard.borrow_mut().take();
    *world.ninja_env_guard.borrow_mut() = Some(override_ninja_env(&env, &script_path));
    Ok(())
}

#[rstest_bdd_macros::given("a fake ninja executable that emits task status lines")]
fn fake_ninja_emits_task_status_lines(world: &TestWorld) -> Result<()> {
    install_fake_ninja(world, &["[1/2] cc -c src/a.c", "[2/2] cc -c src/b.c"])
}

#[rstest_bdd_macros::given("a fake ninja executable that emits malformed task status lines")]
fn fake_ninja_emits_malformed_task_status_lines(world: &TestWorld) -> Result<()> {
    install_fake_ninja(world, &["[x/2] broken", "[2/] broken", "plain output only"])
}

#[rstest_bdd_macros::given("a fake ninja executable that emits stdout output")]
fn fake_ninja_emits_stdout_output(world: &TestWorld) -> Result<()> {
    install_fake_ninja(
        world,
        &[
            "[1/2] cc -c src/a.c",
            "NINJA_STDOUT_MARKER_LINE_1",
            "[2/2] cc -c src/b.c",
            "NINJA_STDOUT_MARKER_LINE_2",
        ],
    )
}
