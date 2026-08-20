//! Integration tests for the advanced usage chapter workflows.
//!
//! These tests cover edge cases and unhappy paths not reached by the BDD
//! scenarios in `tests/features/advanced_usage.feature`. They validate
//! the documented behaviour of `clean`, `graph`, `generate`, configuration
//! layering, and JSON diagnostics.

use anyhow::{Context, Result, ensure};
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

fn assert_json_subcommand_success(
    context: &str,
    command: &str,
    make_ninja: impl FnOnce() -> Result<(TempDir, std::path::PathBuf)>,
) -> Result<()> {
    let workspace = setup_minimal_workspace(Path::new(env!("CARGO_MANIFEST_DIR")), context)?;
    let (_ninja_dir, ninja_path) = make_ninja()?;
    let output = run_netsuke(
        workspace.path(),
        &["--json", command],
        Some(ninja_path.as_path()),
    )?;
    assert_json_success(&output, command)
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
    assert_json_subcommand_success("JSON build success", "build", fake_ninja_check_build_file)
}

#[cfg(unix)]
#[rstest]
fn clean_json_dispatches_tool_and_emits_success_result() -> Result<()> {
    assert_json_subcommand_success("JSON clean success", "clean", || {
        fake_ninja_expect_tool(ToolName::new("clean"))
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
