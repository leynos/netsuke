//! Integration tests for Netsuke tool subcommands.
//!
//! Covers the `clean` and `graph` subcommands which invoke Ninja tools via
//! `ninja -t <tool>`.

use anyhow::{Context, Result, bail, ensure};
use netsuke::cli::{Cli, Commands};
use netsuke::runner::run;
use rstest::{fixture, rstest};
use std::path::PathBuf;
use test_support::{
    check_ninja::{self, ToolName},
    env::{NinjaEnvGuard, SystemEnv, override_ninja_env},
};

mod common;
use common::create_test_manifest;

// Re-export `common::ninja_with_exit_code` as a local fixture so rstest can
// discover it in this integration test crate.
#[fixture]
fn ninja_with_exit_code(
    #[default(0u8)] exit_code: u8,
) -> Result<(tempfile::TempDir, PathBuf, NinjaEnvGuard)> {
    common::ninja_with_exit_code(exit_code)
}

/// Helper: test that a command fails when ninja exits with non-zero status.
fn assert_ninja_failure_propagates(command: Commands) -> Result<()> {
    let (_ninja_dir, _ninja_path, _guard) = ninja_with_exit_code(7)?;
    let (temp, manifest_path) = create_test_manifest()?;
    let cli = Cli {
        file: manifest_path.clone(),
        directory: Some(temp.path().to_path_buf()),
        command: Some(command),
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

fn assert_subcommand_succeeds_without_persisting_file(
    fixture: Result<(tempfile::TempDir, PathBuf, NinjaEnvGuard)>,
    command: Commands,
    name: &'static str,
) -> Result<()> {
    let (_ninja_dir, _ninja_path, _guard) = fixture?;
    let (temp, manifest_path) = create_test_manifest()?;
    let cli = Cli {
        file: manifest_path.clone(),
        directory: Some(temp.path().to_path_buf()),
        command: Some(command),
        ..Cli::default()
    };

    run(&cli).with_context(|| format!("expected {name} subcommand to succeed"))?;

    ensure!(
        !temp.path().join("build.ninja").exists(),
        "{name} subcommand should not leave build.ninja in project directory"
    );
    Ok(())
}

fn assert_subcommand_fails_with_invalid_manifest(
    command: Commands,
    name: &'static str,
) -> Result<()> {
    let temp = tempfile::tempdir().context("create temp dir for invalid manifest test")?;
    let manifest_path = temp.path().join("Netsukefile");
    std::fs::copy("tests/data/invalid_version.yml", &manifest_path)
        .with_context(|| format!("copy invalid manifest to {}", manifest_path.display()))?;
    let cli = Cli {
        file: manifest_path.clone(),
        command: Some(command),
        ..Cli::default()
    };

    let Err(err) = run(&cli) else {
        bail!("expected {name} to fail for invalid manifest");
    };
    let messages: Vec<String> = err.chain().map(ToString::to_string).collect();
    ensure!(
        messages.iter().any(|m| m.contains("loading manifest at")),
        "error should mention manifest loading, got: {messages:?}"
    );
    Ok(())
}

type NinjaToolFixture = fn() -> Result<(tempfile::TempDir, PathBuf, NinjaEnvGuard)>;

/// Fixture: point `NINJA_ENV` at a fake `ninja` that expects `-t clean`.
///
/// Returns: (tempdir holding ninja, path to ninja, `NINJA_ENV` guard)
#[fixture]
fn ninja_expecting_clean() -> Result<(tempfile::TempDir, PathBuf, NinjaEnvGuard)> {
    let (ninja_dir, ninja_path) = check_ninja::fake_ninja_expect_tool(ToolName::new("clean"))?;
    let env = SystemEnv::new();
    let guard = override_ninja_env(&env, ninja_path.as_path());
    Ok((ninja_dir, ninja_path, guard))
}

/// Fixture: point `NINJA_ENV` at a fake `ninja` that expects `-t graph`.
///
/// Returns: (tempdir holding ninja, path to ninja, `NINJA_ENV` guard)
#[fixture]
fn ninja_expecting_graph() -> Result<(tempfile::TempDir, PathBuf, NinjaEnvGuard)> {
    let (ninja_dir, ninja_path) = check_ninja::fake_ninja_expect_tool(ToolName::new("graph"))?;
    let env = SystemEnv::new();
    let guard = override_ninja_env(&env, ninja_path.as_path());
    Ok((ninja_dir, ninja_path, guard))
}

#[cfg(unix)]
#[rstest]
fn run_clean_fails_with_failing_ninja() -> Result<()> {
    assert_ninja_failure_propagates(Commands::Clean)
}

#[cfg(unix)]
#[rstest]
fn run_graph_fails_with_failing_ninja() -> Result<()> {
    assert_ninja_failure_propagates(Commands::Graph)
}

#[cfg(unix)]
#[rstest]
#[case(
    Some(ninja_expecting_clean as NinjaToolFixture),
    Commands::Clean,
    "clean"
)]
#[case(None, Commands::Clean, "clean")]
#[case(
    Some(ninja_expecting_graph as NinjaToolFixture),
    Commands::Graph,
    "graph"
)]
#[case(None, Commands::Graph, "graph")]
fn run_tool_subcommand_table_cases(
    #[case] fixture: Option<NinjaToolFixture>,
    #[case] command: Commands,
    #[case] name: &'static str,
) -> Result<()> {
    match fixture {
        Some(factory) => {
            assert_subcommand_succeeds_without_persisting_file(factory(), command, name)
        }
        None => assert_subcommand_fails_with_invalid_manifest(command, name),
    }
}
