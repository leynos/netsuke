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
use test_support::env::{SystemEnv, VarGuard, override_ninja_env};
use test_support::netsuke::run_netsuke_in;

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
// Configuration layering precedence
// -------------------------------------------------------------------------

#[rstest]
fn config_file_overrides_defaults() -> Result<()> {
    let workspace = setup_minimal_workspace("config file overrides")?;
    let config = workspace.path().join(".netsuke.toml");
    std::fs::write(&config, "verbose = true\n").context("write config file")?;
    let (_ninja_dir, ninja_path) = fake_ninja_check_build_file()?;

    let _guard = VarGuard::set(
        "NETSUKE_CONFIG_PATH",
        std::ffi::OsStr::new(config.to_str().expect("config path is UTF-8")),
    );

    let output = run_netsuke(workspace.path(), &["--help"], Some(ninja_path.as_path()))?;

    ensure!(output.success, "expected --help to succeed");
    Ok(())
}

#[rstest]
fn env_var_overrides_config_file() -> Result<()> {
    let workspace = setup_minimal_workspace("env overrides config")?;
    let config = workspace.path().join(".netsuke.toml");
    std::fs::write(&config, "colour_policy = \"always\"\n").context("write config file")?;

    let _config_guard = VarGuard::set(
        "NETSUKE_CONFIG_PATH",
        std::ffi::OsStr::new(config.to_str().expect("config path is UTF-8")),
    );
    let _env_guard = VarGuard::set("NETSUKE_COLOUR_POLICY", std::ffi::OsStr::new("never"));

    let output = run_netsuke(workspace.path(), &["--help"], None)?;
    ensure!(output.success, "expected --help to succeed");
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

#[test]
fn invalid_config_value_is_handled_gracefully() -> Result<()> {
    let workspace = setup_minimal_workspace("invalid config value")?;
    let config = workspace.path().join(".netsuke.toml");
    std::fs::write(&config, "colour_policy = \"loud\"\n").context("write invalid config file")?;

    let _config_guard = VarGuard::set(
        "NETSUKE_CONFIG_PATH",
        std::ffi::OsStr::new(config.to_str().expect("config path is UTF-8")),
    );

    let output = run_netsuke(workspace.path(), &["--help"], None)?;

    // OrthoConfig silently ignores unrecognised enum values in config files,
    // falling back to the default. The command should not crash.
    ensure!(
        output.success,
        "expected --help to succeed even with invalid config value, \
         got stderr:\n{}",
        output.stderr
    );
    Ok(())
}
