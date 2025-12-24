//! Integration tests for CLI execution using `assert_cmd`.
//!
//! These tests exercise end-to-end command handling by invoking the compiled
//! binary and verifying file outputs for the `manifest` subcommand and the
//! `--emit` build option.

use anyhow::{Context, Result, ensure};
use assert_cmd::Command;
use rstest::rstest;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{TempDir, tempdir};
use test_support::fake_ninja;

/// Builds a `PATH` value that prioritises the provided directory (containing a
/// fake `ninja` implementation) ahead of the existing `PATH`.
fn path_with_fake_ninja(ninja_dir: &tempfile::TempDir) -> Result<OsString> {
    let original = std::env::var_os("PATH").unwrap_or_default();
    std::env::join_paths(
        std::iter::once(ninja_dir.path().to_path_buf()).chain(std::env::split_paths(&original)),
    )
    .context("construct PATH with fake ninja")
}

/// Creates a temporary directory containing a minimal `Netsukefile`.
fn setup_simple_workspace(context: &str) -> Result<TempDir> {
    let temp = tempdir().with_context(|| format!("create temp dir for {context}"))?;
    let netsukefile = temp.path().join("Netsukefile");
    fs::copy("tests/data/minimal.yml", &netsukefile)
        .with_context(|| format!("copy manifest to {} for {context}", netsukefile.display()))?;
    Ok(temp)
}

/// Creates a temporary directory containing a `work/` subdirectory, with the
/// minimal `Netsukefile` written inside that subdirectory.
fn setup_workspace_with_subdir(context: &str) -> Result<(TempDir, PathBuf)> {
    let temp = tempdir().with_context(|| format!("create temp dir for {context}"))?;
    let workdir = temp.path().join("work");
    fs::create_dir_all(&workdir).with_context(|| format!("create work directory for {context}"))?;
    let netsukefile = workdir.join("Netsukefile");
    fs::copy("tests/data/minimal.yml", &netsukefile)
        .with_context(|| format!("copy manifest to {} for {context}", netsukefile.display()))?;
    Ok((temp, workdir))
}

/// Creates a `netsuke` command configured to run from `current_dir`, with the
/// provided `PATH` override.
fn create_netsuke_command(current_dir: &Path, path_override: OsString) -> Command {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("netsuke");
    cmd.current_dir(current_dir).env("PATH", path_override);
    cmd
}

#[derive(Copy, Clone, Debug)]
enum WritesFileCase {
    Manifest,
    BuildEmit,
}

impl WritesFileCase {
    const fn context(self) -> &'static str {
        match self {
            Self::Manifest => "manifest test",
            Self::BuildEmit => "build test",
        }
    }

    const fn args(self) -> &'static [&'static str] {
        match self {
            Self::Manifest => &["manifest"],
            Self::BuildEmit => &["build", "--emit"],
        }
    }

    const fn output_file(self) -> &'static str {
        match self {
            Self::Manifest => "standalone.ninja",
            Self::BuildEmit => "emitted.ninja",
        }
    }

    const fn needs_ninja(self) -> bool {
        matches!(self, Self::BuildEmit)
    }

    const fn expectation(self) -> &'static str {
        match self {
            Self::Manifest => "manifest command should create output file",
            Self::BuildEmit => "build --emit should create emitted manifest",
        }
    }
}

#[rstest]
#[case(WritesFileCase::Manifest)]
#[case(WritesFileCase::BuildEmit)]
fn subcommand_writes_file(#[case] case: WritesFileCase) -> Result<()> {
    let temp = setup_simple_workspace(case.context())?;
    let output = temp.path().join(case.output_file());

    let (ninja_dir_guard, path) = if case.needs_ninja() {
        let (ninja_dir, _ninja_path) = fake_ninja(0u8)?;
        let path = path_with_fake_ninja(&ninja_dir)?;
        (Some(ninja_dir), path)
    } else {
        (None, OsString::from(""))
    };

    let _ninja_dir_guard = ninja_dir_guard;
    let mut cmd = create_netsuke_command(temp.path(), path);
    cmd.args(case.args()).arg(&output).assert().success();

    ensure!(output.exists(), "{}", case.expectation());
    Ok(())
}

#[test]
fn manifest_subcommand_streams_to_stdout_when_dash() -> Result<()> {
    let temp = setup_simple_workspace("manifest stdout test")?;
    let mut cmd = create_netsuke_command(temp.path(), OsString::from(""));
    let output = cmd
        .arg("manifest")
        .arg("-")
        .output()
        .context("run netsuke manifest -")?;
    ensure!(output.status.success(), "manifest - should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("rule ") && stdout.contains("build "),
        "manifest - should print Ninja content, got: {stdout}"
    );
    ensure!(
        !temp.path().join("-").exists(),
        "manifest - should not create a file named '-'"
    );
    Ok(())
}

#[derive(Copy, Clone, Debug)]
enum RelativeOutputCase {
    Manifest,
    BuildEmit,
}

impl RelativeOutputCase {
    const fn context(self) -> &'static str {
        match self {
            Self::Manifest => "manifest -C test",
            Self::BuildEmit => "build -C test",
        }
    }

    const fn args(self) -> &'static [&'static str] {
        match self {
            Self::Manifest => &["-C", "work", "manifest"],
            Self::BuildEmit => &["-C", "work", "build", "--emit"],
        }
    }

    const fn output_file(self) -> &'static str {
        match self {
            Self::Manifest => "out.ninja",
            Self::BuildEmit => "emitted.ninja",
        }
    }

    const fn needs_ninja(self) -> bool {
        matches!(self, Self::BuildEmit)
    }

    const fn should_exist_expectation(self) -> &'static str {
        match self {
            Self::Manifest => "manifest output should be written relative to -C directory",
            Self::BuildEmit => "build --emit should write output relative to -C directory",
        }
    }

    const fn should_not_exist_expectation(self) -> &'static str {
        match self {
            Self::Manifest => "manifest output should not be written outside -C directory",
            Self::BuildEmit => "build --emit should not write output outside -C directory",
        }
    }
}

#[rstest]
#[case(RelativeOutputCase::Manifest)]
#[case(RelativeOutputCase::BuildEmit)]
fn subcommand_resolves_output_relative_to_directory(
    #[case] case: RelativeOutputCase,
) -> Result<()> {
    let (temp, workdir) = setup_workspace_with_subdir(case.context())?;

    let (ninja_dir_guard, path) = if case.needs_ninja() {
        let (ninja_dir, _ninja_path) = fake_ninja(0u8)?;
        let path = path_with_fake_ninja(&ninja_dir)?;
        (Some(ninja_dir), path)
    } else {
        (None, OsString::from(""))
    };

    let _ninja_dir_guard = ninja_dir_guard;
    let mut cmd = create_netsuke_command(temp.path(), path);
    cmd.args(case.args())
        .arg(case.output_file())
        .assert()
        .success();

    ensure!(
        workdir.join(case.output_file()).exists(),
        "{}",
        case.should_exist_expectation()
    );
    ensure!(
        !temp.path().join(case.output_file()).exists(),
        "{}",
        case.should_not_exist_expectation()
    );
    Ok(())
}

#[test]
fn manifest_subcommand_streams_to_stdout_when_dash_with_directory() -> Result<()> {
    let (temp, workdir) = setup_workspace_with_subdir("manifest stdout -C test")?;
    let mut cmd = create_netsuke_command(temp.path(), OsString::from(""));
    let output = cmd
        .arg("-C")
        .arg("work")
        .arg("manifest")
        .arg("-")
        .output()
        .context("run netsuke -C work manifest -")?;
    ensure!(output.status.success(), "manifest - with -C should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("rule ") && stdout.contains("build "),
        "manifest - with -C should print Ninja content, got: {stdout}"
    );
    ensure!(
        !temp.path().join("-").exists(),
        "manifest - with -C should not create a file named '-' in the working directory"
    );
    ensure!(
        !workdir.join("-").exists(),
        "manifest - with -C should not create a file named '-' in the -C directory"
    );
    Ok(())
}
