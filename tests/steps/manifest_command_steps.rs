//! Step definitions for `netsuke manifest` behavioural tests.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros rebind capture names"
)]

use crate::CliWorld;
use anyhow::{Context, Result, ensure};
use assert_cmd::Command;
use cucumber::{given, then, when};
use std::fs;

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
    let temp = world
        .temp
        .as_ref()
        .context("temp dir has not been initialised")?;
    fs::create_dir_all(temp.path().join(&name))
        .with_context(|| format!("create directory {}", temp.path().join(&name).display()))?;
    Ok(())
}

#[when(expr = "the netsuke manifest subcommand is run with {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String arguments"
)]
fn run_manifest_subcommand(world: &mut CliWorld, output: String) -> Result<()> {
    let temp = world
        .temp
        .as_ref()
        .context("temp dir has not been initialised")?;
    let mut cmd = Command::cargo_bin("netsuke").context("locate netsuke binary")?;
    let result = cmd
        .current_dir(temp.path())
        .env("PATH", "")
        .arg("manifest")
        .arg(&output)
        .output()
        .context("run netsuke manifest subcommand")?;

    world.command_stdout = Some(String::from_utf8_lossy(&result.stdout).into_owned());
    world.command_stderr = Some(String::from_utf8_lossy(&result.stderr).into_owned());
    world.run_status = Some(result.status.success());
    world.run_error = if result.status.success() {
        None
    } else {
        world.command_stderr.clone()
    };
    Ok(())
}

#[then(expr = "stdout should contain {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String arguments"
)]
fn stdout_should_contain(world: &mut CliWorld, fragment: String) -> Result<()> {
    let stdout = world
        .command_stdout
        .as_ref()
        .context("no stdout captured from netsuke CLI process")?;
    ensure!(
        stdout.contains(&fragment),
        "expected stdout to contain '{fragment}', got '{stdout}'"
    );
    Ok(())
}

#[then(expr = "stderr should contain {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String arguments"
)]
fn stderr_should_contain(world: &mut CliWorld, fragment: String) -> Result<()> {
    let stderr = world
        .command_stderr
        .as_ref()
        .context("no stderr captured from netsuke CLI process")?;
    ensure!(
        stderr.contains(&fragment),
        "expected stderr to contain '{fragment}', got '{stderr}'"
    );
    Ok(())
}

#[then(expr = "the file {string} should exist")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String arguments"
)]
fn file_should_exist(world: &mut CliWorld, name: String) -> Result<()> {
    let temp = world
        .temp
        .as_ref()
        .context("temp dir has not been initialised")?;
    ensure!(
        temp.path().join(&name).exists(),
        "expected file {} to exist",
        temp.path().join(&name).display()
    );
    Ok(())
}

#[then(expr = "the file {string} should not exist")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String arguments"
)]
fn file_should_not_exist(world: &mut CliWorld, name: String) -> Result<()> {
    let temp = world
        .temp
        .as_ref()
        .context("temp dir has not been initialised")?;
    ensure!(
        !temp.path().join(&name).exists(),
        "expected file {} to not exist",
        temp.path().join(&name).display()
    );
    Ok(())
}
