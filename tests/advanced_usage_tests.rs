//! Integration tests for the advanced usage chapter workflows.
//!
//! These tests cover edge cases and unhappy paths not reached by the BDD
//! scenarios in `tests/features/advanced_usage.feature`. They validate
//! the documented behaviour of `clean`, `graph`, `generate`, configuration
//! layering, and JSON diagnostics.

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::rstest;
use serde_json::Value;
use std::path::Path;
use tempfile::{TempDir, tempdir};
use test_support::check_ninja::fake_ninja_check_build_file;
#[cfg(unix)]
use test_support::check_ninja::{ToolName, fake_ninja_expect_tool};
use test_support::fixture::setup_minimal_workspace;
use test_support::fs as test_fs;
use test_support::netsuke::run_netsuke_in_with_env;

#[path = "advanced_usage/config_precedence.rs"]
mod config_precedence;

/// Captured output from a netsuke invocation.
struct CommandOutput {
    stdout: String,
    stderr: String,
    success: bool,
}

/// Run `netsuke` in `current_dir` with supplied args and optional `NINJA_ENV`.
fn run_netsuke(
    current_dir: &Path,
    args: &[&str],
    ninja_env: Option<&Path>,
) -> Result<CommandOutput> {
    // Build environment variable list based on ninja_env parameter.
    // When ninja_env is provided, pass it via NINJA_ENV to the isolated runner.
    let ninja_env_owned = ninja_env.map(|p| p.to_string_lossy().into_owned());
    let extra_env: Vec<(&str, &str)> = ninja_env_owned
        .as_ref()
        .map(|s| vec![("NETSUKE_NINJA", s.as_str())])
        .unwrap_or_default();
    let run = run_netsuke_in_with_env(current_dir, args, &extra_env)?;
    Ok(CommandOutput {
        stdout: run.stdout,
        stderr: run.stderr,
        success: run.success,
    })
}

/// Run `netsuke` with explicit extra environment and an optional `NINJA_ENV`.
fn run_netsuke_with_env(
    current_dir: &Path,
    args: &[&str],
    ninja_env: Option<&Path>,
    extra_env: &[(&str, &str)],
) -> Result<CommandOutput> {
    let ninja_env_owned = ninja_env.map(|p| p.to_string_lossy().into_owned());
    let mut env_vec: Vec<(&str, &str)> = extra_env.to_vec();
    if let Some(ref s) = ninja_env_owned {
        env_vec.push(("NETSUKE_NINJA", s.as_str()));
    }
    let run = run_netsuke_in_with_env(current_dir, args, &env_vec)?;
    Ok(CommandOutput {
        stdout: run.stdout,
        stderr: run.stderr,
        success: run.success,
    })
}

fn assert_json_success(output: &CommandOutput, expected_command: &str) -> Result<()> {
    ensure!(output.success, "{expected_command} should succeed");
    ensure!(
        output.stderr.is_empty(),
        "{expected_command} should keep stderr empty: {}",
        output.stderr
    );
    let document: Value =
        serde_json::from_str(&output.stdout).context("stdout should be valid JSON")?;
    ensure!(
        document.get("schema_version").and_then(Value::as_u64) == Some(1),
        "JSON result should use schema version 1: {document}"
    );
    ensure!(
        document.pointer("/result/command").and_then(Value::as_str) == Some(expected_command),
        "JSON result should identify the {expected_command} command: {document}"
    );
    ensure!(
        document
            .pointer("/result/content")
            .is_some_and(Value::is_null),
        "JSON result content should be null: {document}"
    );
    Ok(())
}

type NinjaSetup = fn() -> Result<(TempDir, Utf8PathBuf)>;

struct JsonSubcommandRequest {
    context: &'static str,
    command: &'static str,
    make_ninja: NinjaSetup,
}

impl JsonSubcommandRequest {
    const fn into_parts(self) -> (&'static str, &'static str, NinjaSetup) {
        (self.context, self.command, self.make_ninja)
    }
}

fn assert_json_subcommand_success(request: JsonSubcommandRequest) -> Result<()> {
    let (context, command, make_ninja) = request.into_parts();
    let workspace = setup_minimal_workspace(
        Utf8Path::new(env!("CARGO_MANIFEST_DIR")).as_std_path(),
        context,
    )?;
    let workspace_path = utf8_temp_path(&workspace)?;
    let (_ninja_dir, ninja_path) = make_ninja()?;
    let output = run_netsuke(
        workspace_path.as_std_path(),
        &["--json", command],
        Some(ninja_path.as_std_path()),
    )?;
    assert_json_success(&output, command)
}

#[cfg(unix)]
fn make_clean_ninja() -> Result<(TempDir, Utf8PathBuf)> {
    let (ninja_dir, ninja_path) = fake_ninja_expect_tool(ToolName::new("clean"))?;
    Ok((ninja_dir, utf8_path(ninja_path)?))
}

fn make_build_ninja() -> Result<(TempDir, Utf8PathBuf)> {
    let (ninja_dir, ninja_path) = fake_ninja_check_build_file()?;
    Ok((ninja_dir, utf8_path(ninja_path)?))
}

fn utf8_temp_path(temp: &TempDir) -> Result<Utf8PathBuf> {
    utf8_path(temp.path().to_path_buf())
}

fn utf8_path(path: std::path::PathBuf) -> Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path).map_err(|invalid_path| {
        anyhow::anyhow!("test path {} is not UTF-8", invalid_path.display())
    })
}

struct ConfigLayerBuildRequest<'a> {
    context: &'a str,
    config_content: &'a str,
    args: &'a [&'a str],
    extra_env: &'a [(&'a str, &'a str)],
}

impl<'a> ConfigLayerBuildRequest<'a> {
    const fn into_parts(self) -> (&'a str, &'a str, &'a [&'a str], &'a [(&'a str, &'a str)]) {
        (self.context, self.config_content, self.args, self.extra_env)
    }
}

/// Shared workspace setup for configuration-layering tests.
///
/// Creates a minimal workspace, writes `config_content` to `.netsuke.toml`,
/// installs a fake ninja binary, and runs netsuke with the given `args` and
/// `extra_env`.  Returns the captured [`CommandOutput`].
fn run_config_layer_build(request: ConfigLayerBuildRequest<'_>) -> Result<CommandOutput> {
    let (context, config_content, args, extra_env) = request.into_parts();
    let workspace = setup_minimal_workspace(
        Utf8Path::new(env!("CARGO_MANIFEST_DIR")).as_std_path(),
        context,
    )?;
    let workspace_path = utf8_temp_path(&workspace)?;
    let workspace_dir = Dir::open_ambient_dir(&workspace_path, ambient_authority())
        .context("open test workspace")?;
    workspace_dir
        .write(".netsuke.toml", config_content)
        .context("write config file")?;
    let (_ninja_dir, ninja_path) = make_build_ninja()?;
    run_netsuke_with_env(
        workspace_path.as_std_path(),
        args,
        Some(ninja_path.as_std_path()),
        extra_env,
    )
}

// -------------------------------------------------------------------------
// Clean subcommand edge cases
// -------------------------------------------------------------------------

#[cfg(unix)]
#[rstest]
fn clean_without_prior_build_handles_gracefully() -> Result<()> {
    let workspace = setup_minimal_workspace(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "clean without prior build",
    )?;
    let (_ninja_dir, ninja_path) = fake_ninja_expect_tool(ToolName::new("clean"))?;

    let output = run_netsuke(workspace.path(), &["clean"], Some(ninja_path.as_path()))?;

    ensure!(
        output.success,
        "clean without a prior build should dispatch the clean tool successfully: {}",
        output.stderr
    );
    Ok(())
}

#[rstest]
fn build_json_emits_success_result() -> Result<()> {
    assert_json_subcommand_success(JsonSubcommandRequest {
        context: "JSON build success",
        command: "build",
        make_ninja: make_build_ninja,
    })
}

#[cfg(unix)]
#[rstest]
fn clean_json_dispatches_tool_and_emits_success_result() -> Result<()> {
    assert_json_subcommand_success(JsonSubcommandRequest {
        context: "JSON clean success",
        command: "clean",
        make_ninja: make_clean_ninja,
    })
}

// -------------------------------------------------------------------------
// Graph subcommand edge cases
// -------------------------------------------------------------------------

#[test]
fn graph_with_invalid_manifest_fails_with_actionable_error() -> Result<()> {
    let workspace = tempdir().context("create temp dir for graph invalid manifest")?;
    let manifest = workspace.path().join("Netsukefile");
    std::fs::write(&manifest, "not: valid: yaml: [[[").context("write invalid manifest")?;

    let output = run_netsuke(workspace.path(), &["graph"], None)?;

    ensure!(
        !output.success,
        "expected graph with invalid manifest to fail"
    );
    ensure!(
        !output.stderr.is_empty(),
        "expected an error message on stderr"
    );
    Ok(())
}

// -------------------------------------------------------------------------
// Generate subcommand edge cases
// -------------------------------------------------------------------------

#[test]
fn generate_to_unwritable_path_fails_with_path_error() -> Result<()> {
    let workspace = setup_minimal_workspace(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "generate to unwritable path",
    )?;
    // Create a regular file that blocks the parent directory creation.
    let blocker = workspace.path().join("blocker");
    test_fs::write(&blocker, "").context("create blocker file")?;
    let bad_path = blocker.join("out.ninja");

    let output = run_netsuke(
        workspace.path(),
        &[
            "generate",
            "--output",
            bad_path.to_str().expect("path is UTF-8"),
        ],
        None,
    )?;

    ensure!(
        !output.success,
        "expected generate to unwritable path to fail"
    );
    ensure!(
        output.stderr.contains("blocker"),
        "expected path-related error mentioning 'blocker' on stderr, got:\n{}",
        output.stderr
    );
    Ok(())
}

#[test]
fn generate_to_missing_parent_directory_succeeds_by_creating_parents() -> Result<()> {
    let workspace = setup_minimal_workspace(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "generate to missing parent",
    )?;
    // Netsuke automatically creates missing parent directories.
    let nested_path = workspace.path().join("missing_parent").join("out.ninja");

    let output = run_netsuke(
        workspace.path(),
        &[
            "generate",
            "--output",
            nested_path.to_str().expect("path is UTF-8"),
        ],
        None,
    )?;

    ensure!(
        output.success,
        "expected generate to succeed and create parent directories"
    );
    ensure!(
        nested_path.exists(),
        "expected manifest file to be created at {}",
        nested_path.display()
    );
    Ok(())
}

// -------------------------------------------------------------------------

// JSON diagnostics edge cases
// -------------------------------------------------------------------------

#[test]
fn json_diagnostics_with_verbose_produces_valid_json() -> Result<()> {
    let workspace = tempdir().context("create temp dir for JSON diagnostics verbose")?;
    let manifest = workspace.path().join("Netsukefile");
    std::fs::write(&manifest, "not_valid_yaml: [[[").context("write invalid manifest")?;

    let output = run_netsuke(workspace.path(), &["--json", "--verbose", "build"], None)?;

    ensure!(
        !output.success,
        "expected build with invalid manifest to fail"
    );
    // stderr should contain a valid JSON diagnostics envelope (possibly
    // multiline) without tracing noise leaking through.
    let trimmed = output.stderr.trim();
    ensure!(!trimmed.is_empty(), "expected JSON diagnostics on stderr");
    let parsed: serde_json::Value =
        serde_json::from_str(trimmed).context("expected stderr to be a valid JSON document")?;
    ensure!(
        parsed.get("diagnostics").is_some(),
        "expected a 'diagnostics' key in the JSON envelope"
    );
    // stdout should be empty when diagnostics go to stderr
    ensure!(
        output.stdout.trim().is_empty(),
        "expected stdout to be empty with --json, got:\n{}",
        output.stdout
    );
    Ok(())
}

#[test]
fn generate_to_stdout_contains_ninja_rules() -> Result<()> {
    let workspace =
        setup_minimal_workspace(Path::new(env!("CARGO_MANIFEST_DIR")), "generate to stdout")?;

    let output = run_netsuke(workspace.path(), &["generate"], None)?;

    ensure!(output.success, "expected generate to stdout to succeed");
    ensure!(
        output.stdout.contains("rule "),
        "expected stdout to contain Ninja rule statements, got:\n{}",
        output.stdout
    );
    // Progress output goes to stderr; the generated content goes to stdout.
    ensure!(
        !output.stderr.contains("rule "),
        "expected progress messages on stderr, not manifest content, got:\n{}",
        output.stderr
    );
    Ok(())
}
