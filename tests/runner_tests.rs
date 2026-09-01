//! Behavioural tests for the Netsuke runner and CLI integration.

use anyhow::{Context, Result, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use netsuke::cli::{Cli, Commands};
use netsuke::localization::{self, keys};
use netsuke::output_prefs;
use netsuke::runner::{BuildTargets, run, run_ninja, run_ninja_tool, run_with_ninja_program};
use rstest::{fixture, rstest};
use std::path::{Path, PathBuf};
use test_support::{check_ninja, fake_ninja};

#[path = "runner_cases/default_targets.rs"]
mod default_targets;
mod fixtures;

use fixtures::create_test_manifest;

fn utf8_path(path: &Path) -> Result<&Utf8Path> {
    Utf8Path::from_path(path).context("test path is not valid UTF-8")
}

/// Convert a test filesystem path into the UTF-8 CLI boundary type.
fn utf8_path_buf(path: &Path) -> Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path.to_path_buf())
        .map_err(|non_utf8| anyhow::anyhow!("test path is not valid UTF-8: {}", non_utf8.display()))
}

/// Build a `Cli` that runs `generate --output <output>` for `manifest` within
/// `directory`. Shared by the generate-output tests, which differ only in their
/// workspace setup and post-run assertions.
fn generate_cli(manifest: &Path, directory: &Path, output: &Path) -> Result<Cli> {
    Ok(Cli {
        file: utf8_path_buf(manifest)?,
        directory: Some(utf8_path_buf(directory)?),
        command: Some(Commands::Generate {
            output: Some(output.to_path_buf()),
        }),
        ..Cli::default()
    })
}

/// Fixture: provide a fake `ninja` binary with a configurable exit code.
///
/// This is a re-export of `common::ninja_with_exit_code` so `rstest` can
/// discover it in this integration test crate.
///
/// Returns the temporary directory and path to the fake Ninja binary.
#[fixture]
fn ninja_with_exit_code(#[default(0u8)] exit_code: u8) -> Result<(tempfile::TempDir, PathBuf)> {
    fixtures::ninja_with_exit_code(exit_code)
}

/// Fixture: create a fake `ninja` that validates `-f` files.
///
/// Returns the temporary directory and executable path for explicit injection.
#[fixture]
fn checking_ninja() -> Result<(tempfile::TempDir, PathBuf)> {
    let (ninja_dir, ninja_path) = check_ninja::fake_ninja_check_build_file()?;
    Ok((ninja_dir, ninja_path))
}

/// Shared setup for tests that inject a fake Ninja executable path.
///
/// Returns the fake Ninja directory, executable path, temporary project
/// directory, and constructed CLI.
fn setup_ninja_env_test() -> Result<(tempfile::TempDir, PathBuf, tempfile::TempDir, Cli)> {
    let (ninja_dir, ninja_path) = checking_ninja()?;
    let (temp, manifest_path) = create_test_manifest()?;
    let cli = Cli {
        file: utf8_path_buf(&manifest_path)?,
        directory: Some(utf8_path_buf(temp.path())?),
        ..Cli::default()
    };
    Ok((ninja_dir, ninja_path, temp, cli))
}

#[test]
fn run_exits_with_manifest_error_on_invalid_version() -> Result<()> {
    let temp = tempfile::tempdir().context("create temp dir for invalid manifest test")?;
    let manifest_path = temp.path().join("Netsukefile");
    std::fs::copy("tests/data/invalid_version.yml", &manifest_path)
        .with_context(|| format!("copy invalid manifest to {}", manifest_path.display()))?;
    let cli = Cli {
        file: utf8_path_buf(&manifest_path)?,
        ..Cli::default()
    };

    let Err(err) = run(&cli, output_prefs::resolve(None)) else {
        bail!("expected run to fail for invalid manifest");
    };
    let expected = localization::message(keys::RUNNER_CONTEXT_LOAD_MANIFEST)
        .with_arg("path", manifest_path.display().to_string())
        .to_string();
    ensure!(
        err.to_string().contains(&expected),
        "error should mention manifest loading, got: {err}"
    );
    let chain: Vec<String> = err
        .chain()
        .map(|e: &(dyn std::error::Error + '_)| e.to_string())
        .collect();
    let parse_message = localization::message(keys::MANIFEST_PARSE).to_string();
    ensure!(
        chain.iter().any(|s| s.contains(&parse_message)),
        "expected error chain to include parse error, got: {chain:?}"
    );
    Ok(())
}

/// Helper: test that a command fails when ninja exits with non-zero status.
fn assert_ninja_failure_propagates(command: Option<Commands>) -> Result<()> {
    let (_ninja_dir, ninja_path) = ninja_with_exit_code(7)?;
    let (temp, manifest_path) = create_test_manifest()?;
    let cli = Cli {
        file: utf8_path_buf(&manifest_path)?,
        directory: Some(utf8_path_buf(temp.path())?),
        command,
        ..Cli::default()
    };

    let Err(err) =
        run_with_ninja_program(&cli, output_prefs::resolve(None), utf8_path(&ninja_path)?)
    else {
        bail!("expected run to fail when ninja exits non-zero");
    };
    let messages: Vec<String> = err
        .chain()
        .map(|e: &(dyn std::error::Error + '_)| e.to_string())
        .collect();
    ensure!(
        messages.iter().any(|m| m.contains("ninja exited")),
        "error should report ninja exit status, got: {messages:?}"
    );
    Ok(())
}

/// Helper: assert that a function fails with `NotFound` when the ninja binary is missing
fn assert_binary_not_found<F>(f: F) -> Result<()>
where
    F: FnOnce() -> std::io::Result<()>,
{
    let err = f()
        .err()
        .context("expected function to fail when binary is missing")?;
    ensure!(
        err.kind() == std::io::ErrorKind::NotFound,
        "expected NotFound error, got {:?}",
        err.kind()
    );
    Ok(())
}

/// Helper: assert that a runner function fails with `NotFound` when ninja binary is missing.
fn assert_runner_not_found<F>(runner_call: F) -> Result<()>
where
    F: FnOnce(&Cli) -> std::io::Result<()>,
{
    assert_binary_not_found(|| {
        let cli = Cli::default();
        runner_call(&cli)
    })
}

#[rstest]
fn run_ninja_not_found() -> Result<()> {
    assert_runner_not_found(|cli| {
        let targets = BuildTargets::default();
        run_ninja(
            Utf8Path::new("does-not-exist"),
            cli,
            Utf8Path::new("build.ninja"),
            &targets,
        )
    })
}

#[rstest]
fn run_executes_ninja_without_persisting_file() -> Result<()> {
    let (_ninja_dir, ninja_path, temp, cli) = setup_ninja_env_test()?;

    run_with_ninja_program(&cli, output_prefs::resolve(None), utf8_path(&ninja_path)?)
        .context("expected build to succeed")?;

    // Ensure no ninja file remains in project directory
    ensure!(
        !temp.path().join("build.ninja").exists(),
        "build.ninja should not persist after a build"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn run_generate_with_output_keeps_file() -> Result<()> {
    let (temp, manifest_path) = create_test_manifest()?;
    let output_path = temp.path().join("generated.ninja");
    let cli = generate_cli(&manifest_path, temp.path(), &output_path)?;

    run(&cli, output_prefs::resolve(None)).context("expected generate to succeed")?;

    ensure!(
        output_path.exists(),
        "output path should exist after generate"
    );
    let emitted = std::fs::read_to_string(&output_path)
        .with_context(|| format!("read generated Ninja file at {}", output_path.display()))?;
    ensure!(
        emitted.contains("rule "),
        "emitted manifest should include rule section"
    );
    ensure!(
        emitted.contains("build "),
        "emitted manifest should include build statements"
    );
    ensure!(
        !temp.path().join("build.ninja").exists(),
        "generate should not leave build.ninja"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn run_generate_with_output_creates_parent_dirs() -> Result<()> {
    let (temp, manifest_path) = create_test_manifest()?;
    let nested_dir = temp.path().join("nested").join("dir");
    let output_path = nested_dir.join("generated.ninja");
    ensure!(
        !nested_dir.exists(),
        "nested directory should not exist prior to generate"
    );
    let cli = generate_cli(&manifest_path, temp.path(), &output_path)?;

    run(&cli, output_prefs::resolve(None))
        .context("expected generate to succeed with nested output path")?;
    ensure!(output_path.exists(), "output path should be created");
    ensure!(nested_dir.exists(), "nested directory should be created");
    Ok(())
}

#[test]
fn run_generate_subcommand_writes_file() -> Result<()> {
    let (temp, manifest_path) = create_test_manifest()?;
    let output_path = temp.path().join("standalone.ninja");
    let cli = generate_cli(&manifest_path, temp.path(), &output_path)?;

    run(&cli, output_prefs::resolve(None)).context("expected generate subcommand to succeed")?;
    ensure!(
        output_path.exists(),
        "generate command should create output file"
    );
    ensure!(
        !temp.path().join("build.ninja").exists(),
        "generate command should not leave build.ninja"
    );
    Ok(())
}

#[test]
fn run_generate_subcommand_accepts_relative_manifest_path() -> Result<()> {
    let (temp, _manifest_path) = create_test_manifest()?;
    let output_path = temp.path().join("relative.ninja");
    let cli = Cli {
        file: Utf8PathBuf::from("Netsukefile"),
        directory: Some(utf8_path_buf(temp.path())?),
        command: Some(Commands::Generate {
            output: Some(output_path.clone()),
        }),
        ..Cli::default()
    };

    run(&cli, output_prefs::resolve(None))
        .context("expected generate subcommand to accept relative manifest path")?;
    ensure!(
        output_path.exists(),
        "generate command should create output file for relative manifest path"
    );
    Ok(())
}

#[test]
fn run_respects_env_override_for_ninja() -> Result<()> {
    let (_temp_dir_env, ninja_env_path) = fake_ninja(0u8)?;
    let (temp, manifest_path) = create_test_manifest()?;
    let cli = Cli {
        file: utf8_path_buf(&manifest_path)?,
        directory: Some(utf8_path_buf(temp.path())?),
        ..Cli::default()
    };

    run_with_ninja_program(
        &cli,
        output_prefs::resolve(None),
        utf8_path(&ninja_env_path)?,
    )
    .context("expected injected Ninja programme to be used")?;
    Ok(())
}

#[rstest]
fn run_succeeds_with_checking_ninja_env() -> Result<()> {
    let (_ninja_dir, ninja_path, _temp, cli) = setup_ninja_env_test()?;

    run_with_ninja_program(&cli, output_prefs::resolve(None), utf8_path(&ninja_path)?)
        .context("expected run to succeed using NINJA_ENV check binary")?;
    ensure!(ninja_path.exists(), "fake ninja should remain present");
    Ok(())
}

#[rstest]
fn run_fails_with_failing_ninja_env() -> Result<()> {
    assert_ninja_failure_propagates(None)
}

#[rstest]
fn run_ninja_tool_not_found() -> Result<()> {
    assert_runner_not_found(|cli| {
        run_ninja_tool(
            Utf8Path::new("does-not-exist"),
            cli,
            Utf8Path::new("build.ninja"),
            "clean",
        )
    })
}
