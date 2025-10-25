//! Step definitions for Ninja process execution.
// NOTE: These module-level allowances cannot be narrowed while we rely on the
// cucumber macros, which repeatedly shadow captured identifiers and rely on
// `expect()` for concise failure reporting. Once the step suite migrates to
// `rstest-bdd` we can delete these expectations entirely.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros rebind capture names"
)]
#![expect(
    clippy::expect_used,
    reason = "Test steps favour `expect` for compact failure messages"
)]
use crate::CliWorld;
use anyhow::{Context, Result, anyhow, ensure};
use camino::Utf8Path;
use cucumber::{given, then, when};
use netsuke::runner::{self, BuildTargets, NINJA_PROGRAM};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use test_support::{
    check_ninja, ensure_manifest_exists,
    env::{self, EnvMut},
    fake_ninja,
};

/// Installs a test-specific ninja binary and updates the `PATH`.
#[expect(
    clippy::needless_pass_by_value,
    reason = "helper owns path for simplicity"
)]
fn install_test_ninja(
    env: &impl EnvMut,
    world: &mut CliWorld,
    dir: TempDir,
    ninja_path: PathBuf,
) -> Result<()> {
    let guard = env::prepend_dir_to_path(env, dir.path())?;
    world.path_guard = Some(guard);
    world.ninja = Some(ninja_path.to_string_lossy().into_owned());
    world.temp = Some(dir);
    Ok(())
}

/// Creates a fake ninja executable that exits with the given status code.
#[given(expr = "a fake ninja executable that exits with {int}")]
fn install_fake_ninja(world: &mut CliWorld, exit_code: i32) -> Result<()> {
    let exit_code: u8 = u8::try_from(exit_code)
        .expect("exit code must be between 0 and 255 for fake_ninja");
    let (dir, path) = fake_ninja(exit_code)?;
    let env = env::mocked_path_env();
    install_test_ninja(&env, world, dir, path)
}

/// Creates a fake ninja executable that validates the build file path.
#[given("a fake ninja executable that checks for the build file")]
fn fake_ninja_check(world: &mut CliWorld) -> Result<()> {
    let (dir, path) = check_ninja::fake_ninja_check_build_file()?;
    let env = env::mocked_path_env();
    install_test_ninja(&env, world, dir, path)
}

/// Sets up a scenario where no ninja executable is available.
///
/// This step creates a temporary directory and records the path to a
/// non-existent `ninja` binary within that directory, allowing tests to verify
/// behaviour when the executable is missing.
#[given("no ninja executable is available")]
fn no_ninja(world: &mut CliWorld) -> Result<()> {
    let dir = TempDir::new().context("create temp dir for missing ninja scenario")?;
    let path = dir.path().join("ninja");
    let env = env::mocked_path_env();
    install_test_ninja(&env, world, dir, path)
}

/// Updates the CLI to use the temporary directory created for the fake ninja.
#[given("the CLI uses the temporary directory")]
fn cli_uses_temp_dir(world: &mut CliWorld) -> Result<()> {
    let temp = world
        .temp
        .as_ref()
        .context("CLI temp directory has not been initialised")?;
    let cli = world
        .cli
        .as_mut()
        .context("CLI configuration has not been initialised")?;
    cli.directory = Some(temp.path().to_path_buf());
    Ok(())
}

/// Creates a directory named `build.ninja` in the temporary working directory.
#[given("a directory named build.ninja exists")]
fn build_dir_exists(world: &mut CliWorld) -> Result<()> {
    let temp = world
        .temp
        .as_ref()
        .context("CLI temp directory has not been initialised")?;
    fs::create_dir(temp.path().join("build.ninja")).context("create build.ninja directory")?;
    Ok(())
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
fn run(world: &mut CliWorld) -> Result<()> {
    let dir = world
        .temp
        .as_ref()
        .context("CLI temp directory has not been initialised")?;
    {
        let cli = world
            .cli
            .as_mut()
            .context("CLI configuration has not been initialised")?;
        let temp_path = Utf8Path::from_path(dir.path())
            .ok_or_else(|| anyhow!("temporary directory path is not valid UTF-8"))?;
        let manifest_path = Utf8Path::from_path(&cli.file)
            .ok_or_else(|| anyhow!("CLI manifest path is not valid UTF-8"))?;
        let manifest = ensure_manifest_exists(temp_path, manifest_path)
            .context("ensure manifest exists in temp workspace")?;
        cli.file = manifest.into_std_path_buf();
    }
    let program = if let Some(ninja) = &world.ninja {
        Path::new(ninja)
    } else {
        Path::new(NINJA_PROGRAM)
    };
    let targets = BuildTargets::default();
    let cli = world
        .cli
        .as_ref()
        .context("CLI configuration has not been initialised")?;
    match runner::run_ninja(program, cli, Path::new("build.ninja"), &targets) {
        Ok(()) => {
            world.run_status = Some(true);
            world.run_error = None;
        }
        Err(e) => {
            world.run_status = Some(false);
            world.run_error = Some(e.to_string());
        }
    }
    Ok(())
}

/// Asserts that the command succeeds.
#[then("the command should succeed")]
fn command_should_succeed(world: &mut CliWorld) -> Result<()> {
    ensure!(
        world.run_status == Some(true),
        "command run status should be success"
    );
    Ok(())
}

/// Asserts that the command fails and records an error message.
#[then("the command should fail")]
fn command_should_fail(world: &mut CliWorld) -> Result<()> {
    ensure!(
        world.run_status == Some(false),
        "command run status should be failure"
    );
    ensure!(
        world.run_error.is_some(),
        "expected command failure to record an error message"
    );
    Ok(())
}

/// Asserts that the command failed and the error message matches the expected value.
#[then(expr = "the command should fail with error {string}")]
fn command_should_fail_with_error(world: &mut CliWorld, expected_fragment: String) -> Result<()> {
    ensure!(
        world.run_status == Some(false),
        "command run status should be failure"
    );
    let actual = world
        .run_error
        .as_ref()
        .expect("expected an error message, but none was recorded");
    let expected_fragment = expected_fragment.into_boxed_str();
    ensure!(
        actual.contains(&*expected_fragment),
        "expected error message to contain '{expected_fragment}', but was '{actual}'",
    );
    Ok(())
}
