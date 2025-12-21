//! Step definitions for process execution scenarios.

use crate::bdd::fixtures::{RefCellOptionExt, strip_quotes, with_world};
use anyhow::{Context, Result, anyhow, ensure};
use camino::Utf8Path;
use netsuke::runner::{self, BuildTargets, NINJA_PROGRAM};
use rstest_bdd_macros::{given, then, when};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use test_support::{
    check_ninja::{self, ToolName},
    ensure_manifest_exists,
    env::{self, EnvMut},
    fake_ninja,
};

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Installs a test-specific ninja binary and updates the `PATH`.
fn install_test_ninja(env: &impl EnvMut, dir: TempDir, ninja_path: PathBuf) -> Result<()> {
    with_world(|world| {
        let guard = env::prepend_dir_to_path(env, dir.path())?;
        *world.path_guard.borrow_mut() = Some(guard);
        world
            .ninja_content
            .set(ninja_path.to_string_lossy().into_owned());
        world.ninja_env_guard.borrow_mut().take();
        let system_env = env::SystemEnv::new();
        *world.ninja_env_guard.borrow_mut() =
            Some(env::override_ninja_env(&system_env, &ninja_path));
        *world.temp_dir.borrow_mut() = Some(dir);
        Ok(())
    })
}

/// Prepares the CLI for execution by ensuring the manifest exists and updating paths.
fn prepare_cli_with_directory() -> Result<()> {
    with_world(|world| {
        let temp_path = {
            let temp_dir = world.temp_dir.borrow();
            let dir = temp_dir
                .as_ref()
                .context("CLI temp directory has not been initialised")?;
            Utf8Path::from_path(dir.path())
                .ok_or_else(|| anyhow!("temporary directory path is not valid UTF-8"))?
                .to_owned()
        };

        world
            .cli
            .with_mut(|cli| {
                let manifest_path = Utf8Path::from_path(&cli.file)
                    .ok_or_else(|| anyhow!("CLI manifest path is not valid UTF-8"))?;
                let manifest = ensure_manifest_exists(&temp_path, manifest_path)
                    .context("ensure manifest exists in temp workspace")?;
                cli.file = manifest.into_std_path_buf();
                Ok::<(), anyhow::Error>(())
            })
            .context("CLI configuration has not been initialised")??;
        Ok(())
    })
}

/// Prepares the CLI for execution with an absolute file path.
fn prepare_cli_with_absolute_file() -> Result<()> {
    prepare_cli_with_directory()?;
    with_world(|world| {
        world
            .cli
            .with_mut(|cli| {
                cli.directory = None;
            })
            .context("CLI configuration has not been initialised")?;
        Ok(())
    })
}

/// Records the result of a command execution in the test world.
fn record_result(result: Result<(), String>) {
    with_world(|world| match result {
        Ok(()) => {
            world.run_status.set(true);
            world.run_error.clear();
        }
        Err(e) => {
            world.run_status.set(false);
            world.run_error.set(e);
        }
    });
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a fake ninja executable that exits with {code:i32}")]
fn install_fake_ninja_step(code: i32) -> Result<()> {
    let exit_code =
        u8::try_from(code).map_err(|_| anyhow!("exit code must be between 0 and 255"))?;
    let (dir, path) = fake_ninja(exit_code)?;
    let env = env::mocked_path_env();
    install_test_ninja(&env, dir, path)
}

#[given("a fake ninja executable that checks for the build file")]
fn fake_ninja_check() -> Result<()> {
    let (dir, path) = check_ninja::fake_ninja_check_build_file()?;
    let env = env::mocked_path_env();
    install_test_ninja(&env, dir, path)
}

#[cfg(unix)]
#[given("a fake ninja executable that expects the clean tool")]
fn fake_ninja_expects_clean() -> Result<()> {
    let (dir, path) = check_ninja::fake_ninja_expect_tool(ToolName::new("clean"))?;
    let env = env::mocked_path_env();
    install_test_ninja(&env, dir, path)
}

#[cfg(unix)]
#[given("a fake ninja executable that expects clean with {jobs:u32} jobs")]
fn fake_ninja_expects_clean_with_jobs(jobs: u32) -> Result<()> {
    let (dir, path) =
        check_ninja::fake_ninja_expect_tool_with_jobs(ToolName::new("clean"), Some(jobs), None)?;
    let env = env::mocked_path_env();
    install_test_ninja(&env, dir, path)
}

#[cfg(unix)]
#[given("a fake ninja executable that expects the graph tool")]
fn fake_ninja_expects_graph() -> Result<()> {
    let (dir, path) = check_ninja::fake_ninja_expect_tool(ToolName::new("graph"))?;
    let env = env::mocked_path_env();
    install_test_ninja(&env, dir, path)
}

#[cfg(unix)]
#[given("a fake ninja executable that expects graph with {jobs:u32} jobs")]
fn fake_ninja_expects_graph_with_jobs(jobs: u32) -> Result<()> {
    let (dir, path) =
        check_ninja::fake_ninja_expect_tool_with_jobs(ToolName::new("graph"), Some(jobs), None)?;
    let env = env::mocked_path_env();
    install_test_ninja(&env, dir, path)
}

#[given("no ninja executable is available")]
fn no_ninja() -> Result<()> {
    let dir = TempDir::new().context("create temp dir for missing ninja scenario")?;
    let path = dir.path().join("ninja");
    let env = env::mocked_path_env();
    install_test_ninja(&env, dir, path)
}

#[given("the CLI uses the temporary directory")]
fn cli_uses_temp_dir() -> Result<()> {
    with_world(|world| {
        let temp_path = {
            let temp = world.temp_dir.borrow();
            temp.as_ref()
                .context("CLI temp directory has not been initialised")?
                .path()
                .to_path_buf()
        };
        world
            .cli
            .with_mut(|cli| {
                cli.directory = Some(temp_path);
            })
            .context("CLI configuration has not been initialised")?;
        Ok(())
    })
}

#[given("a directory named build.ninja exists")]
fn build_dir_exists() -> Result<()> {
    with_world(|world| {
        let temp = world.temp_dir.borrow();
        let dir = temp
            .as_ref()
            .context("CLI temp directory has not been initialised")?;
        fs::create_dir(dir.path().join("build.ninja")).context("create build.ninja directory")?;
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the ninja process is run")]
fn run() -> Result<()> {
    prepare_cli_with_directory()?;
    let result = with_world(|world| {
        let ninja_path = world.ninja_content.get();
        let program_path: PathBuf;
        let program = if let Some(ref ninja) = ninja_path {
            program_path = PathBuf::from(ninja.as_str());
            program_path.as_path()
        } else {
            Path::new(NINJA_PROGRAM)
        };
        let targets = BuildTargets::default();
        world
            .cli
            .with_ref(|cli| runner::run_ninja(program, cli, Path::new("build.ninja"), &targets))
            .ok_or_else(|| "CLI configuration has not been initialised".to_string())?
            .map_err(|e| e.to_string())
    });
    record_result(result);
    Ok(())
}

#[cfg(unix)]
fn run_subcommand() -> Result<()> {
    prepare_cli_with_absolute_file()?;
    let result = with_world(|world| {
        world
            .cli
            .with_ref(|cli| runner::run(cli))
            .ok_or_else(|| "CLI configuration has not been initialised".to_string())?
            .map_err(|e| format!("{e:#}"))
    });
    record_result(result);
    Ok(())
}

#[cfg(unix)]
#[when("the clean process is run")]
fn run_clean() -> Result<()> {
    run_subcommand()
}

#[cfg(unix)]
#[when("the graph process is run")]
fn run_graph() -> Result<()> {
    run_subcommand()
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the command should succeed")]
fn command_should_succeed() -> Result<()> {
    with_world(|world| {
        ensure!(
            world.run_status.get() == Some(true),
            "command run status should be success"
        );
        Ok(())
    })
}

#[then("the command should fail")]
fn command_should_fail() -> Result<()> {
    with_world(|world| {
        ensure!(
            world.run_status.get() == Some(false),
            "command run status should be failure"
        );
        ensure!(
            world.run_error.is_filled(),
            "expected command failure to record an error message"
        );
        Ok(())
    })
}

#[then("the command should fail with error {fragment}")]
fn command_should_fail_with_error(fragment: String) -> Result<()> {
    let fragment = strip_quotes(&fragment);
    with_world(|world| {
        ensure!(
            world.run_status.get() == Some(false),
            "command run status should be failure"
        );
        let actual = world
            .run_error
            .get()
            .context("expected an error message, but none was recorded")?;
        ensure!(
            actual.contains(fragment),
            "expected error message to contain '{fragment}', but was '{actual}'",
        );
        Ok(())
    })
}
