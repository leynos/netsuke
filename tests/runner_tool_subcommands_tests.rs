//! Integration tests for Netsuke tool subcommands.
//!
//! Covers the `clean` and `graph` subcommands which invoke Ninja tools via
//! `ninja -t <tool>`.

use anyhow::{Context, Result, bail, ensure};
use netsuke::cli::{Cli, Commands};
use netsuke::cli_localization;
use netsuke::localization::{self, keys};
use netsuke::runner::run;
use rstest::{fixture, rstest};
use std::path::PathBuf;
use std::sync::Arc;
use test_support::{
    check_ninja::{self, ToolName},
    env::{NinjaEnvGuard, SystemEnv, override_ninja_env},
};

mod fixtures;
use fixtures::create_test_manifest;

fn set_en_localizer() -> localization::LocalizerGuard {
    let localizer = cli_localization::build_localizer(Some("en-US"));
    localization::set_localizer_for_tests(Arc::from(localizer))
}

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
    fixtures::ninja_with_exit_code(exit_code)
}

/// Helper: test that a command fails when ninja exits with non-zero status.
fn assert_ninja_failure_propagates(command: Commands) -> Result<()> {
    let _guard = set_en_localizer();
    let (_ninja_dir, _ninja_path, _ninja_guard) = ninja_with_exit_code(7)?;
    let (temp, manifest_path) = create_test_manifest()?;
    let expected_tool = match &command {
        Commands::Clean => "clean",
        Commands::Graph => "graph",
        other => bail!("unsupported command for this helper: {other:?}"),
    };
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
    ensure!(
        messages
            .iter()
            .any(|m| m.contains(&format!("-t {expected_tool}"))),
        "error should mention running ninja tool {expected_tool}, got: {messages:?}"
    );
    ensure!(
        messages
            .iter()
            .any(|m| m.contains("with build file") && m.contains(".ninja")),
        "error should include build file context, got: {messages:?}"
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
    let _guard = set_en_localizer();
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
    let expected = localization::message(keys::RUNNER_CONTEXT_LOAD_MANIFEST)
        .with_arg("path", manifest_path.display().to_string())
        .to_string();
    ensure!(
        messages.iter().any(|m| m.contains(&expected)),
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
