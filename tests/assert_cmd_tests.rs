//! End-to-end CLI coverage for generated Ninja output.

use anyhow::{Context, Result, ensure};
use assert_cmd::Command;
use netsuke::locale_resolution::{LocaleEnvProvider, SystemEnv};
use predicates::prelude::*;
use rstest::rstest;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{TempDir, tempdir};

fn setup_simple_workspace(context: &str) -> Result<TempDir> {
    let temp = tempdir().with_context(|| format!("create temp dir for {context}"))?;
    let netsukefile = temp.path().join("Netsukefile");
    fs::copy("tests/data/minimal.yml", &netsukefile)
        .with_context(|| format!("copy manifest to {} for {context}", netsukefile.display()))?;
    Ok(temp)
}

fn setup_workspace_with_subdir(context: &str) -> Result<(TempDir, PathBuf)> {
    let temp = tempdir().with_context(|| format!("create temp dir for {context}"))?;
    let workdir = temp.path().join("work");
    fs::create_dir_all(&workdir).with_context(|| format!("create work directory for {context}"))?;
    fs::copy("tests/data/minimal.yml", workdir.join("Netsukefile"))
        .with_context(|| format!("copy manifest for {context}"))?;
    Ok((temp, workdir))
}

/// Build a `netsuke` command whose child environment this test alone decides.
///
/// Clearing first is what makes an assertion here a statement about the code
/// rather than about the ambient shell. `NETSUKE_FILE` would otherwise pick a
/// different manifest, and `NETSUKE_JSON` would route a human-output test
/// through the JSON renderer, so either would fail a row for a reason no commit
/// introduced. With `HOME`, `XDG_CONFIG_HOME`, and the rest unset, the child
/// also discovers no operator configuration, so only the manifest each test
/// stages can affect the result.
///
/// `PATH` is forwarded because the child still has to locate executables a
/// manifest names, and `SystemRoot` because a Windows process needs it to
/// start; both are platform plumbing rather than test inputs. They are read
/// through the project's environment seam rather than `std::env` so the lint
/// that forbids direct environment access still holds in tests.
fn create_netsuke_command(current_dir: &Path) -> Command {
    let mut command = assert_cmd::cargo::cargo_bin_cmd!("netsuke");
    command.current_dir(current_dir).env_clear();
    if let Some(path) = SystemEnv.var("PATH") {
        command.env("PATH", path);
    }
    if cfg!(windows)
        && let Some(system_root) = SystemEnv.var("SystemRoot")
    {
        command.env("SystemRoot", system_root);
    }
    command
}

#[cfg(unix)]
#[rstest]
fn build_rejects_command_injecting_glob_filename() -> Result<()> {
    let temp = tempdir().context("create command-injection test workspace")?;
    let files = temp.path().join("files");
    fs::create_dir(&files).context("create glob input directory")?;
    fs::write(
        files.join("a.txt; touch PWNED; #.txt"),
        "attacker-controlled filename",
    )
    .context("write malicious glob input")?;
    let pattern = files.join("*.txt");
    let manifest = format!(
        concat!(
            "netsuke_version: '1.0.0'\n",
            "targets:\n",
            "  - foreach: glob({pattern:?})\n",
            "    name: output-{{{{ index }}}}.txt\n",
            "    command: echo {{{{ item }}}}\n"
        ),
        pattern = pattern.display()
    );
    fs::write(temp.path().join("Netsukefile"), manifest)
        .context("write command-injection test manifest")?;

    let output = create_netsuke_command(temp.path())
        .args(["--json", "build"])
        .output()
        .context("run Netsuke build with JSON diagnostics")?;
    ensure!(
        !output.status.success(),
        "the unsafe filename must stop the Netsuke build"
    );
    let stderr = String::from_utf8(output.stderr).context("build stderr should be valid UTF-8")?;
    ensure!(
        stderr.contains("characters that require shell quoting"),
        "the failed build must report the shell-quoting rejection: {stderr}"
    );
    ensure!(
        !temp.path().join("PWNED").exists(),
        "the attacker-controlled filename must not create PWNED"
    );
    Ok(())
}

fn assert_generate_streams_to_stdout(
    current_dir: &Path,
    args: &[&str],
    command_description: &str,
) -> Result<()> {
    let output = create_netsuke_command(current_dir)
        .args(args)
        .output()
        .with_context(|| format!("run {command_description}"))?;
    ensure!(
        output.status.success(),
        "{command_description} should succeed"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("rule ") && stdout.contains("build "),
        "{command_description} should print Ninja content, got: {stdout}"
    );
    Ok(())
}

#[rstest]
fn generate_writes_file() -> Result<()> {
    let temp = setup_simple_workspace("generate file test")?;
    let output = temp.path().join("standalone.ninja");

    create_netsuke_command(temp.path())
        .args(["generate", "--output"])
        .arg(&output)
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    ensure!(output.exists(), "generate should create the output file");
    let contents = fs::read_to_string(&output)
        .with_context(|| format!("read generated file {}", output.display()))?;
    ensure!(
        contents.contains("rule ") && contents.contains("build "),
        "generated file should contain Ninja content, got: {contents}"
    );
    Ok(())
}

/// Cover the manifest schema-version gate through the real binary.
#[path = "assert_cmd_tests/schema_version.rs"]
mod schema_version;

/// Provide the shared process fixtures and cover the missing-manifest diagnostic.
#[path = "assert_cmd_tests/runner_diagnostics.rs"]
mod runner_diagnostics;

/// Snapshot the human-readable causes of manifest, graph, and Ninja failures.
#[path = "assert_cmd_tests/human_failure_causes.rs"]
mod human_failure_causes;

/// Cover launch diagnostics for every subcommand and rendering mode.
#[path = "assert_cmd_tests/launch_diagnostics.rs"]
mod launch_diagnostics;

/// Cover failed-recipe diagnostics at the same process boundary.
#[path = "assert_cmd_tests/ninja_exit_diagnostics.rs"]
mod ninja_exit_diagnostics;

/// Detect the temporary build file the diagnostics must not name.
#[path = "assert_cmd_tests/generated_build_file.rs"]
mod generated_build_file;

#[rstest]
fn generate_streams_to_stdout_by_default() -> Result<()> {
    let temp = setup_simple_workspace("generate stdout test")?;
    assert_generate_streams_to_stdout(temp.path(), &["generate"], "netsuke generate")
}

#[rstest]
fn generate_resolves_output_relative_to_directory() -> Result<()> {
    let (temp, workdir) = setup_workspace_with_subdir("generate -C test")?;

    create_netsuke_command(temp.path())
        .args(["-C", "work", "generate", "--output", "out.ninja"])
        .assert()
        .success();

    ensure!(
        workdir.join("out.ninja").exists(),
        "generate output should be written relative to -C directory"
    );
    ensure!(
        !temp.path().join("out.ninja").exists(),
        "generate output should not be written outside -C directory"
    );
    Ok(())
}

#[rstest]
fn generate_streams_to_stdout_with_directory() -> Result<()> {
    let (temp, _workdir) = setup_workspace_with_subdir("generate stdout -C test")?;
    assert_generate_streams_to_stdout(
        temp.path(),
        &["-C", "work", "generate"],
        "netsuke -C work generate",
    )
}
