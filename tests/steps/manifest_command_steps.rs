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
use std::ops::Deref;
use std::path::PathBuf;

#[derive(Debug)]
struct DirectoryName(String);

impl From<String> for DirectoryName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::str::FromStr for DirectoryName {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_owned()))
    }
}

impl Deref for DirectoryName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

#[derive(Debug)]
struct FileName(String);

impl From<String> for FileName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::str::FromStr for FileName {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_owned()))
    }
}

impl Deref for FileName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

#[derive(Debug)]
struct ManifestOutput(String);

impl From<String> for ManifestOutput {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::str::FromStr for ManifestOutput {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_owned()))
    }
}

impl Deref for ManifestOutput {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

#[derive(Debug)]
struct OutputFragment(String);

impl From<String> for OutputFragment {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::str::FromStr for OutputFragment {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_owned()))
    }
}

impl Deref for OutputFragment {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

fn get_temp_path(world: &CliWorld) -> Result<PathBuf> {
    let temp = world
        .temp
        .as_ref()
        .context("temp dir has not been initialised")?;
    Ok(temp.path().to_path_buf())
}

fn assert_output_contains(
    output: Option<&String>,
    output_name: &str,
    fragment: &str,
) -> Result<()> {
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
fn directory_named_exists(world: &mut CliWorld, name: DirectoryName) -> Result<()> {
    let DirectoryName(name) = name;
    let temp_path = get_temp_path(world)?;
    let dir_path = temp_path.join(name.as_str());
    fs::create_dir_all(&dir_path)
        .with_context(|| format!("create directory {}", dir_path.display()))?;
    Ok(())
}

#[when(expr = "the netsuke manifest subcommand is run with {string}")]
fn run_manifest_subcommand(world: &mut CliWorld, output: ManifestOutput) -> Result<()> {
    let ManifestOutput(output) = output;
    let temp_path = get_temp_path(world)?;
    let mut cmd = Command::cargo_bin("netsuke").context("locate netsuke binary")?;
    let result = cmd
        .current_dir(temp_path)
        .env("PATH", "")
        .arg("manifest")
        .arg(output.as_str())
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
fn stdout_should_contain(world: &mut CliWorld, fragment: OutputFragment) -> Result<()> {
    let OutputFragment(fragment) = fragment;
    assert_output_contains(world.command_stdout.as_ref(), "stdout", fragment.as_str())
}

#[then(expr = "stderr should contain {string}")]
fn stderr_should_contain(world: &mut CliWorld, fragment: OutputFragment) -> Result<()> {
    let OutputFragment(fragment) = fragment;
    assert_output_contains(world.command_stderr.as_ref(), "stderr", fragment.as_str())
}

#[then(expr = "the file {string} should exist")]
fn file_should_exist(world: &mut CliWorld, name: FileName) -> Result<()> {
    let FileName(name) = name;
    assert_file_existence(world, name.as_str(), true)
}

#[then(expr = "the file {string} should not exist")]
fn file_should_not_exist(world: &mut CliWorld, name: FileName) -> Result<()> {
    let FileName(name) = name;
    assert_file_existence(world, name.as_str(), false)
}
