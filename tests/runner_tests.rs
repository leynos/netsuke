//! Behavioural tests for the Netsuke runner and CLI integration.

use anyhow::{Context, Result, bail, ensure};
use netsuke::cli::{BuildArgs, Cli, Commands};
use netsuke::runner::{BuildTargets, run, run_ninja, run_ninja_tool};
use rstest::{fixture, rstest};
use std::path::{Path, PathBuf};
use test_support::{
    check_ninja,
    env::{NinjaEnvGuard, SystemEnv, override_ninja_env, prepend_dir_to_path},
    fake_ninja,
};

mod common;
use common::{create_test_manifest, workflow_contents};

/// Fixture: provide a fake `ninja` binary with a configurable exit code.
///
/// This is a re-export of `common::ninja_with_exit_code` so `rstest` can
/// discover it in this integration test crate.
///
/// Returns: (`tempfile::TempDir`, path to the ninja binary, `NinjaEnvGuard`)
#[fixture]
fn ninja_with_exit_code(
    #[default(0u8)] exit_code: u8,
) -> Result<(tempfile::TempDir, PathBuf, NinjaEnvGuard)> {
    common::ninja_with_exit_code(exit_code)
}

/// Fixture: point `NINJA_ENV` at a fake `ninja` that validates `-f` files.
///
/// Using `NINJA_ENV` avoids mutating `PATH`, letting tests run in parallel
/// without trampling each other's environment.
///
/// Returns: (tempdir holding ninja, `NINJA_ENV` guard)
#[fixture]
fn ninja_in_env() -> Result<(tempfile::TempDir, PathBuf, NinjaEnvGuard)> {
    let (ninja_dir, ninja_path) = check_ninja::fake_ninja_check_build_file()?;
    let env = SystemEnv::new();
    let guard = override_ninja_env(&env, ninja_path.as_path());
    Ok((ninja_dir, ninja_path, guard))
}

/// Shared setup for tests that rely on `NINJA_ENV`.
///
/// Returns the fake ninja directory, temp project directory, constructed CLI,
/// and the guard keeping `NINJA_ENV` set for the test duration.
fn setup_ninja_env_test() -> Result<(
    tempfile::TempDir,
    PathBuf,
    tempfile::TempDir,
    Cli,
    NinjaEnvGuard,
)> {
    let (ninja_dir, ninja_path, guard) = ninja_in_env()?;
    let (temp, manifest_path) = create_test_manifest()?;
    let cli = Cli {
        file: manifest_path.clone(),
        directory: Some(temp.path().to_path_buf()),
        ..Cli::default()
    };
    Ok((ninja_dir, ninja_path, temp, cli, guard))
}

#[test]
fn unit_workflow_contents_reads_release_workflow() {
    let _contents = workflow_contents("release.yml").expect("release workflow should be readable");
}

#[test]
fn run_exits_with_manifest_error_on_invalid_version() -> Result<()> {
    let temp = tempfile::tempdir().context("create temp dir for invalid manifest test")?;
    let manifest_path = temp.path().join("Netsukefile");
    std::fs::copy("tests/data/invalid_version.yml", &manifest_path)
        .with_context(|| format!("copy invalid manifest to {}", manifest_path.display()))?;
    let cli = Cli {
        file: manifest_path.clone(),
        ..Cli::default()
    };

    let Err(err) = run(&cli) else {
        bail!("expected run to fail for invalid manifest");
    };
    ensure!(
        err.to_string().contains("loading manifest at"),
        "error should mention manifest loading, got: {err}"
    );
    let chain: Vec<String> = err.chain().map(ToString::to_string).collect();
    ensure!(
        chain.iter().any(|s| s.contains("manifest parse error")),
        "expected error chain to include 'manifest parse error', got: {chain:?}"
    );
    Ok(())
}

/// Helper: test that a command fails when ninja exits with non-zero status.
fn assert_ninja_failure_propagates(command: Option<Commands>) -> Result<()> {
    let (_ninja_dir, _ninja_path, _guard) = ninja_with_exit_code(7)?;
    let (temp, manifest_path) = create_test_manifest()?;
    let cli = Cli {
        file: manifest_path.clone(),
        directory: Some(temp.path().to_path_buf()),
        command,
        ..Cli::default()
    };

    let Err(err) = run(&cli) else {
        bail!("expected run to fail when ninja exits non-zero");
    };
    let messages: Vec<String> = err.chain().map(ToString::to_string).collect();
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
            Path::new("does-not-exist"),
            cli,
            Path::new("build.ninja"),
            &targets,
        )
    })
}

#[rstest]
fn run_executes_ninja_without_persisting_file() -> Result<()> {
    let (_ninja_dir, _ninja_path, temp, cli, _guard) = setup_ninja_env_test()?;

    run(&cli).context("expected run to succeed without emit path")?;

    // Ensure no ninja file remains in project directory
    ensure!(
        !temp.path().join("build.ninja").exists(),
        "build.ninja should not persist when emit path unset"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn run_build_with_emit_keeps_file() -> Result<()> {
    let (_ninja_dir, _ninja_path, _guard) = ninja_in_env()?;
    let (temp, manifest_path) = create_test_manifest()?;
    let emit_path = temp.path().join("emitted.ninja");
    let cli = Cli {
        file: manifest_path.clone(),
        directory: Some(temp.path().to_path_buf()),
        command: Some(Commands::Build(BuildArgs {
            emit: Some(emit_path.clone()),
            targets: Vec::new(),
        })),
        ..Cli::default()
    };

    run(&cli).context("expected run to succeed with emit path")?;

    ensure!(emit_path.exists(), "emit path should exist after build");
    let emitted = std::fs::read_to_string(&emit_path)
        .with_context(|| format!("read emitted ninja at {}", emit_path.display()))?;
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
        "build.ninja should not remain when emit path provided"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn run_build_with_emit_creates_parent_dirs() -> Result<()> {
    let (_ninja_dir, _ninja_path, _guard) = ninja_with_exit_code(0)?;
    let (temp, manifest_path) = create_test_manifest()?;
    let nested_dir = temp.path().join("nested").join("dir");
    let emit_path = nested_dir.join("emitted.ninja");
    ensure!(
        !nested_dir.exists(),
        "nested directory should not exist prior to build"
    );
    let cli = Cli {
        file: manifest_path.clone(),
        directory: Some(temp.path().to_path_buf()),
        command: Some(Commands::Build(BuildArgs {
            emit: Some(emit_path.clone()),
            targets: Vec::new(),
        })),
        ..Cli::default()
    };

    run(&cli).context("expected run to succeed with nested emit path")?;
    ensure!(emit_path.exists(), "emit path should be created");
    ensure!(nested_dir.exists(), "nested directory should be created");
    Ok(())
}

#[test]
fn run_manifest_subcommand_writes_file() -> Result<()> {
    let (temp, manifest_path) = create_test_manifest()?;
    let output_path = temp.path().join("standalone.ninja");
    let cli = Cli {
        file: manifest_path.clone(),
        directory: Some(temp.path().to_path_buf()),
        command: Some(Commands::Manifest {
            file: output_path.clone(),
        }),
        ..Cli::default()
    };

    run(&cli).context("expected manifest subcommand to succeed")?;
    ensure!(
        output_path.exists(),
        "manifest command should create output file"
    );
    ensure!(
        !temp.path().join("build.ninja").exists(),
        "manifest command should not leave build.ninja"
    );
    Ok(())
}

#[test]
fn run_manifest_subcommand_accepts_relative_manifest_path() -> Result<()> {
    let (temp, _manifest_path) = create_test_manifest()?;
    let output_path = temp.path().join("relative.ninja");
    let cli = Cli {
        file: PathBuf::from("Netsukefile"),
        directory: Some(temp.path().to_path_buf()),
        command: Some(Commands::Manifest {
            file: output_path.clone(),
        }),
        ..Cli::default()
    };

    run(&cli).context("expected manifest subcommand to accept relative manifest path")?;
    ensure!(
        output_path.exists(),
        "manifest command should create output file for relative manifest path"
    );
    Ok(())
}

#[test]
fn run_respects_env_override_for_ninja() -> Result<()> {
    let (_temp_dir_env, ninja_env_path) = fake_ninja(0u8)?;
    let (temp_dir_path, _ninja_path_on_path) = fake_ninja(1u8)?;
    let env = SystemEnv::new();
    let _path_guard =
        prepend_dir_to_path(&env, temp_dir_path.path()).context("prepend failing ninja to PATH")?;
    let _env_guard = override_ninja_env(&env, &ninja_env_path);
    let (temp, manifest_path) = create_test_manifest()?;
    let cli = Cli {
        file: manifest_path.clone(),
        directory: Some(temp.path().to_path_buf()),
        ..Cli::default()
    };

    run(&cli).context("expected run to prefer NINJA_ENV over PATH entry")?;
    Ok(())
}

#[rstest]
fn run_succeeds_with_checking_ninja_env() -> Result<()> {
    let (_ninja_dir, ninja_path, _temp, cli, _guard) = setup_ninja_env_test()?;

    run(&cli).context("expected run to succeed using NINJA_ENV check binary")?;
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
            Path::new("does-not-exist"),
            cli,
            Path::new("build.ninja"),
            "clean",
        )
    })
}
