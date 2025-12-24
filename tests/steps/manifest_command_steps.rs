//! Step definitions for `netsuke manifest` behavioural tests.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros rebind capture names"
)]

use crate::CliWorld;
use anyhow::{Context, Result, ensure};
use cucumber::{given, then, when};
use std::fs;
use std::path::PathBuf;
use test_support::netsuke::run_netsuke_in;

/// Captured output from a command execution.
struct CommandOutput {
    stdout: String,
    stderr: String,
}

fn record_run(world: &mut CliWorld, output: CommandOutput, success: bool) {
    world.command_stdout = Some(output.stdout);
    world.command_stderr = Some(output.stderr);
    world.run_status = Some(success);
    world.run_error = if success {
        None
    } else {
        world.command_stderr.clone()
    };
}

fn get_temp_path(world: &CliWorld) -> Result<PathBuf> {
    let temp = world
        .temp
        .as_ref()
        .context("temp dir has not been initialised")?;
    Ok(temp.path().to_path_buf())
}

fn assert_output_contains(output: Option<&str>, output_name: &str, fragment: &str) -> Result<()> {
    let output =
        output.with_context(|| format!("no {output_name} captured from netsuke CLI process"))?;
    ensure!(
        output.contains(fragment),
        "expected {output_name} to contain '{fragment}', got '{output}'"
    );
    Ok(())
}

fn assert_file_existence(world: &CliWorld, name: &str, should_exist: bool) -> Result<()> {
    let temp_path = get_temp_path(world)?;
    let path = temp_path.join(name);
    let expected = if should_exist { "exist" } else { "not exist" };
    ensure!(
        path.exists() == should_exist,
        "expected file {} to {expected}",
        path.display()
    );
    Ok(())
}

#[given("a minimal Netsuke workspace")]
fn minimal_workspace(world: &mut CliWorld) -> Result<()> {
    let temp = tempfile::tempdir().context("create temp dir for manifest workspace")?;
    let netsukefile = temp.path().join("Netsukefile");
    fs::copy("tests/data/minimal.yml", &netsukefile)
        .with_context(|| format!("copy manifest to {}", netsukefile.display()))?;
    world.temp = Some(temp);
    world.run_status = None;
    world.run_error = None;
    world.command_stdout = None;
    world.command_stderr = None;
    Ok(())
}

#[given(expr = "a directory named {string} exists")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String arguments"
)]
fn directory_named_exists(world: &mut CliWorld, name: String) -> Result<()> {
    let temp_path = get_temp_path(world)?;
    let dir_path = temp_path.join(&name);
    fs::create_dir_all(&dir_path)
        .with_context(|| format!("create directory {}", dir_path.display()))?;
    Ok(())
}

#[when(expr = "the netsuke manifest subcommand is run with {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String arguments"
)]
fn run_manifest_subcommand(world: &mut CliWorld, output: String) -> Result<()> {
    let temp_path = get_temp_path(world)?;
    let args = ["manifest", &output];
    let run = run_netsuke_in(temp_path.as_path(), &args)?;
    record_run(
        world,
        CommandOutput {
            stdout: run.stdout,
            stderr: run.stderr,
        },
        run.success,
    );
    Ok(())
}

#[then(expr = "stdout should contain {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String arguments"
)]
fn stdout_should_contain(world: &mut CliWorld, fragment: String) -> Result<()> {
    assert_output_contains(world.command_stdout.as_deref(), "stdout", &fragment)
}

#[then(expr = "stderr should contain {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String arguments"
)]
fn stderr_should_contain(world: &mut CliWorld, fragment: String) -> Result<()> {
    assert_output_contains(world.command_stderr.as_deref(), "stderr", &fragment)
}

#[then(expr = "the file {string} should exist")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String arguments"
)]
fn file_should_exist(world: &mut CliWorld, name: String) -> Result<()> {
    assert_file_existence(world, &name, true)
}

#[then(expr = "the file {string} should not exist")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String arguments"
)]
fn file_should_not_exist(world: &mut CliWorld, name: String) -> Result<()> {
    assert_file_existence(world, &name, false)
}
