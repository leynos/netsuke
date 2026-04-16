//! Integration tests for the advanced usage chapter workflows.
//!
//! These tests cover edge cases and unhappy paths not reached by the BDD
//! scenarios in `tests/features/advanced_usage.feature`. They validate
//! the documented behaviour of `clean`, `graph`, `manifest`, configuration
//! layering, and JSON diagnostics.

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::path::Path;
use tempfile::{TempDir, tempdir};
use test_support::check_ninja::fake_ninja_check_build_file;
use test_support::env::{SystemEnv, override_ninja_env};
use test_support::netsuke::{run_netsuke_in, run_netsuke_in_with_env};

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
    let _guard = ninja_env.map(|path| override_ninja_env(&SystemEnv::new(), path));
    let run = run_netsuke_in(current_dir, args)?;
    Ok(CommandOutput {
        stdout: run.stdout,
        stderr: run.stderr,
        success: run.success,
    })
}

/// Run `netsuke` in `current_dir` with supplied args, optional `NINJA_ENV`,
/// and explicit extra environment variables.
///
/// Unlike [`run_netsuke`], this variant passes env vars directly to the child
/// process via `env_clear()` + explicit forwarding, avoiding process-level
/// `VarGuard` mutations that race under parallel test execution.
fn run_netsuke_with_env(
    current_dir: &Path,
    args: &[&str],
    ninja_env: Option<&Path>,
    extra_env: &[(&str, &str)],
) -> Result<CommandOutput> {
    let _guard = ninja_env.map(|path| override_ninja_env(&SystemEnv::new(), path));
    let run = run_netsuke_in_with_env(current_dir, args, extra_env)?;
    Ok(CommandOutput {
        stdout: run.stdout,
        stderr: run.stderr,
        success: run.success,
    })
}

fn setup_minimal_workspace(context: &str) -> Result<TempDir> {
    let temp = tempdir().with_context(|| format!("create temp dir for {context}"))?;
    let manifest = temp.path().join("Netsukefile");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/minimal.yml");
    std::fs::copy(&source, &manifest)
        .with_context(|| format!("copy {} to {}", source.display(), manifest.display()))?;
    Ok(temp)
}

/// Shared workspace setup for configuration-layering tests.
///
/// Creates a minimal workspace, writes `config_content` to `.netsuke.toml`,
/// installs a fake ninja binary, and runs netsuke with the given `args` and
/// `extra_env`.  Returns the captured [`CommandOutput`].
fn run_config_layer_build(
    context: &str,
    config_content: &str,
    args: &[&str],
    extra_env: &[(&str, &str)],
) -> Result<CommandOutput> {
    let workspace = setup_minimal_workspace(context)?;
    let config = workspace.path().join(".netsuke.toml");
    std::fs::write(&config, config_content).context("write config file")?;
    let (_ninja_dir, ninja_path) = fake_ninja_check_build_file()?;
    run_netsuke_with_env(
        workspace.path(),
        args,
        Some(ninja_path.as_path()),
        extra_env,
    )
}

// -------------------------------------------------------------------------
// Clean subcommand edge cases
// -------------------------------------------------------------------------

#[test]
fn clean_without_prior_build_handles_gracefully() -> Result<()> {
    let workspace = setup_minimal_workspace("clean without prior build")?;
    let (_ninja_dir, ninja_path) = fake_ninja_check_build_file()?;

    let output = run_netsuke(workspace.path(), &["clean"], Some(ninja_path.as_path()))?;

    // Clean in a workspace that has never been built should either succeed
    // as a no-op or fail with a clear message about missing build state.
    // The actual behaviour depends on ninja; either outcome is acceptable.
    ensure!(
        output.success || output.stderr.contains("build"),
        "expected clean to succeed or fail with a clear build-related message, \
         got stderr:\n{}",
        output.stderr
    );
    Ok(())
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
// Manifest subcommand edge cases
// -------------------------------------------------------------------------

#[test]
fn manifest_to_unwritable_path_fails_with_path_error() -> Result<()> {
    let workspace = setup_minimal_workspace("manifest to unwritable path")?;
    // Create a regular file that blocks the parent directory creation.
    let blocker = workspace.path().join("blocker");
    std::fs::write(&blocker, "").context("create blocker file")?;
    let bad_path = blocker.join("out.ninja");

    let output = run_netsuke(
        workspace.path(),
        &["manifest", bad_path.to_str().expect("path is UTF-8")],
        None,
    )?;

    ensure!(
        !output.success,
        "expected manifest to unwritable path to fail"
    );
    ensure!(
        output.stderr.contains("blocker"),
        "expected path-related error mentioning 'blocker' on stderr, got:\n{}",
        output.stderr
    );
    Ok(())
}

// -------------------------------------------------------------------------
// Configuration layering precedence
// -------------------------------------------------------------------------

#[rstest]
#[case(&["build"], &[], true)]
#[case(&["build"], &[("NETSUKE_VERBOSE", "false")], false)]
#[case(&["--verbose", "build"], &[("NETSUKE_VERBOSE", "false")], true)]
fn verbose_config_precedence(
    #[case] args: &[&str],
    #[case] extra_env: &[(&str, &str)],
    #[case] expect_timing: bool,
) -> Result<()> {
    let output = run_config_layer_build(
        "config precedence test",
        "verbose = true\n",
        args,
        extra_env,
    )?;
    ensure!(output.success, "expected build to succeed");
    if expect_timing {
        ensure!(
            output.stderr.contains("Timing"),
            "expected verbose timing summary in stderr, got:\n{}",
            output.stderr
        );
    } else {
        ensure!(
            !output.stderr.contains("Timing"),
            "expected no timing summary, got:\n{}",
            output.stderr
        );
    }
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

    let output = run_netsuke(
        workspace.path(),
        &["--diag-json", "--verbose", "build"],
        None,
    )?;

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
        "expected stdout to be empty with --diag-json, got:\n{}",
        output.stdout
    );
    Ok(())
}

#[test]
fn manifest_to_stdout_contains_ninja_rules() -> Result<()> {
    let workspace = setup_minimal_workspace("manifest to stdout")?;

    let output = run_netsuke(workspace.path(), &["manifest", "-"], None)?;

    ensure!(output.success, "expected manifest to stdout to succeed");
    ensure!(
        output.stdout.contains("rule "),
        "expected stdout to contain Ninja rule statements, got:\n{}",
        output.stdout
    );
    Ok(())
}

/// An invalid enum value in a config file produces a clear validation error
/// rather than crashing with an unhelpful message.
#[test]
fn invalid_config_value_reports_validation_error() -> Result<()> {
    let workspace = setup_minimal_workspace("invalid config value")?;
    let config = workspace.path().join(".netsuke.toml");
    std::fs::write(&config, "colour_policy = \"loud\"\n").context("write invalid config file")?;

    let output = run_netsuke_with_env(workspace.path(), &["manifest", "-"], None, &[])?;

    ensure!(
        !output.success,
        "expected manifest with invalid config to fail"
    );
    // The error message should mention the invalid value and valid options.
    ensure!(
        output.stderr.contains("loud"),
        "expected error to mention the invalid value 'loud', got:\n{}",
        output.stderr
    );
    ensure!(
        output.stderr.contains("auto") && output.stderr.contains("always"),
        "expected error to list valid options, got:\n{}",
        output.stderr
    );
    Ok(())
}
