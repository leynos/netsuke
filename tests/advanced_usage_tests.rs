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
    std::fs::copy("tests/data/minimal.yml", &manifest)
        .with_context(|| format!("copy minimal manifest to {}", manifest.display()))?;
    Ok(temp)
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

/// Config file sets `verbose = true`; assert the build emits a timing summary.
///
/// The `.netsuke.toml` is placed in the workspace directory so netsuke's
/// project-scope discovery finds it without needing `NETSUKE_CONFIG_PATH`.
#[rstest]
fn config_file_overrides_defaults() -> Result<()> {
    let workspace = setup_minimal_workspace("config file overrides")?;
    let config = workspace.path().join(".netsuke.toml");
    std::fs::write(&config, "verbose = true\n").context("write config file")?;
    let (_ninja_dir, ninja_path) = fake_ninja_check_build_file()?;

    let output = run_netsuke_with_env(
        workspace.path(),
        &["build"],
        Some(ninja_path.as_path()),
        &[],
    )?;

    ensure!(output.success, "expected build to succeed");
    // verbose = true in the config file should produce a timing summary
    ensure!(
        output.stderr.contains("Timing"),
        "expected verbose timing summary in stderr (config should override default), \
         got:\n{}",
        output.stderr
    );
    Ok(())
}

/// Config file sets `verbose = true`, env sets `NETSUKE_VERBOSE = false`.
/// The environment should win: no timing summary in output.
#[rstest]
fn env_var_overrides_config_file() -> Result<()> {
    let workspace = setup_minimal_workspace("env overrides config")?;
    let config = workspace.path().join(".netsuke.toml");
    std::fs::write(&config, "verbose = true\n").context("write config file")?;
    let (_ninja_dir, ninja_path) = fake_ninja_check_build_file()?;

    let output = run_netsuke_with_env(
        workspace.path(),
        &["build"],
        Some(ninja_path.as_path()),
        &[("NETSUKE_VERBOSE", "false")],
    )?;

    ensure!(output.success, "expected build to succeed");
    // env var verbose=false should override the config file's verbose=true
    ensure!(
        !output.stderr.contains("Timing"),
        "expected no timing summary (env should override config), got:\n{}",
        output.stderr
    );
    Ok(())
}

/// Verify the full three-tier precedence ladder: CLI > env > config file.
///
/// The config file sets `verbose = true`, the environment sets
/// `NETSUKE_VERBOSE=false` (overriding the file), and the CLI passes
/// `--verbose` (overriding the environment). The CLI flag should win,
/// producing a timing summary in stderr.
#[rstest]
fn cli_flag_overrides_env_which_overrides_config_file() -> Result<()> {
    let workspace = setup_minimal_workspace("full precedence ladder")?;
    let config = workspace.path().join(".netsuke.toml");
    std::fs::write(&config, "verbose = true\n").context("write config file")?;
    let (_ninja_dir, ninja_path) = fake_ninja_check_build_file()?;

    // env says false, overriding the config file's true;
    // CLI says --verbose, overriding the env's false
    let output = run_netsuke_with_env(
        workspace.path(),
        &["--verbose", "build"],
        Some(ninja_path.as_path()),
        &[("NETSUKE_VERBOSE", "false")],
    )?;

    ensure!(output.success, "expected verbose build to succeed");
    // Verbose mode emits a timing summary containing "Timing"
    ensure!(
        output.stderr.contains("Timing"),
        "expected verbose timing summary in stderr (CLI should override env), \
         got:\n{}",
        output.stderr
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
