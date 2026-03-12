//! Integration tests verifying that log output is written to stderr.
//!
//! These tests exercise the production logging path by invoking the compiled
//! binary and asserting log messages appear on stderr rather than stdout.

use anyhow::{Context, Result, ensure};
use predicates::prelude::*;
use rstest::rstest;
use serde_json::Value;
use std::fs;
use std::path::Path;
use tempfile::tempdir;
use test_support::env::{SystemEnv, override_ninja_env};

#[cfg(unix)]
fn make_script_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .with_context(|| format!("read metadata for {}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("set executable bit for {}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn make_script_executable(_path: &Path) -> Result<()> {
    Ok(())
}

fn write_fake_ninja_script(
    path: &Path,
    stdout_lines: &[&str],
    stderr_marker: Option<&str>,
) -> Result<()> {
    let script = if cfg!(windows) {
        let mut script = String::from("@echo off\r\n");
        for line in stdout_lines {
            script.push_str("echo ");
            script.push_str(line);
            script.push_str("\r\n");
        }
        if let Some(marker) = stderr_marker {
            script.push_str("echo ");
            script.push_str(marker);
            script.push_str(" 1>&2\r\n");
        }
        script.push_str("exit /B 0\r\n");
        script
    } else {
        let mut script = String::from(
            "#!/bin/sh\nwhile IFS= read -r line; do\n  printf '%s\\n' \"$line\"\ndone <<'NETSUKE_OUTPUT'\n",
        );
        for line in stdout_lines {
            script.push_str(line);
            script.push('\n');
        }
        script.push_str("NETSUKE_OUTPUT\n");
        if let Some(marker) = stderr_marker {
            script.push_str("printf '%s\\n' '");
            script.push_str(marker);
            script.push_str("' >&2\n");
        }
        script.push_str("exit 0\n");
        script
    };

    fs::write(path, script)
        .with_context(|| format!("write fake ninja script {}", path.display()))?;
    make_script_executable(path)
}

/// Verifies that runner errors are logged to stderr.
///
/// The test creates an empty temporary directory (no manifest) and runs the
/// `graph` subcommand, which fails quickly. The error log should appear on
/// stderr, not stdout.
#[test]
fn main_logs_errors_to_stderr() {
    let temp = tempdir().expect("create temp dir");
    // ManifestNotFound errors are rendered via miette with diagnostic output.
    assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("graph")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found in"))
        .stdout(predicate::str::contains("Netsukefile").not());
}

#[test]
fn diag_json_failures_emit_single_json_document_on_stderr() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("--diag-json")
        .arg("graph")
        .output()
        .context("run netsuke with --diag-json")?;

    ensure!(!output.status.success(), "expected command failure");
    ensure!(
        output.stdout.is_empty(),
        "stdout should remain empty on failure"
    );

    let stderr = String::from_utf8(output.stderr).context("stderr should be valid UTF-8")?;
    let value: Value = serde_json::from_str(&stderr).context("stderr should be valid JSON")?;
    let schema_version = value
        .get("schema_version")
        .and_then(Value::as_i64)
        .context("JSON diagnostics should include schema_version")?;
    let diagnostics = value
        .get("diagnostics")
        .and_then(Value::as_array)
        .context("JSON diagnostics should include a diagnostics array")?;
    let diagnostic_code = diagnostics
        .first()
        .and_then(|diagnostic| diagnostic.get("code"))
        .and_then(Value::as_str)
        .context("first diagnostic should include a code")?;
    ensure!(
        schema_version == 1,
        "JSON diagnostics should include the schema version",
    );
    ensure!(
        diagnostic_code == "netsuke::runner::manifest_not_found",
        "missing manifest should map to the runner diagnostic code",
    );
    ensure!(
        !stderr.contains("ERROR"),
        "stderr should not contain tracing or text-mode prefixes: {stderr}",
    );
    Ok(())
}

#[test]
fn diag_json_success_keeps_stdout_artifact_and_stderr_empty() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let manifest_path = temp.path().join("Netsukefile");
    fs::copy("tests/data/minimal.yml", &manifest_path)
        .with_context(|| format!("copy manifest to {}", manifest_path.display()))?;

    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("--diag-json")
        .arg("manifest")
        .arg("-")
        .output()
        .context("run netsuke manifest with --diag-json")?;

    ensure!(output.status.success(), "expected command success");
    ensure!(
        output.stderr.is_empty(),
        "stderr should remain empty on success"
    );

    let stdout = String::from_utf8(output.stdout).context("stdout should be valid UTF-8")?;
    ensure!(
        stdout.contains("build hello: "),
        "stdout should contain the generated Ninja manifest, got:\n{stdout}",
    );
    Ok(())
}

/// Asserts that `--diag-json <flag>` produces human-readable stdout and an
/// empty stderr (i.e. Clap's built-in handlers are not affected by JSON mode).
fn assert_diag_json_passthrough(flag: &str, ctx: &str, stdout_marker: &str) -> Result<()> {
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .arg("--diag-json")
        .arg(flag)
        .output()
        .with_context(|| format!("run netsuke --diag-json {flag}"))?;

    ensure!(output.status.success(), "{ctx} should exit successfully");
    let stdout = String::from_utf8(output.stdout).context("stdout should be valid UTF-8")?;
    ensure!(
        stdout.contains(stdout_marker),
        "{ctx} output should remain human-readable"
    );
    ensure!(
        output.stderr.is_empty(),
        "{ctx} should not emit diagnostics JSON"
    );
    Ok(())
}

#[rstest]
#[case("--help", "help", "Usage:")]
#[case("--version", "version", "netsuke")]
fn diag_json_passthrough_uses_normal_clap_output(
    #[case] flag: &str,
    #[case] ctx: &str,
    #[case] stdout_marker: &str,
) -> Result<()> {
    assert_diag_json_passthrough(flag, ctx, stdout_marker)
}

#[test]
fn config_driven_diag_json_formats_merge_failures_as_json() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let config_path = temp.path().join("netsuke.toml");
    fs::write(&config_path, "diag_json = true\njobs = \"many\"\n")
        .with_context(|| format!("write config {}", config_path.display()))?;

    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .env("NETSUKE_CONFIG_PATH", &config_path)
        .output()
        .context("run netsuke with invalid config")?;

    ensure!(!output.status.success(), "expected merge failure");
    ensure!(
        output.stdout.is_empty(),
        "stdout should remain empty on merge failure"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be valid UTF-8")?;
    let value: Value = serde_json::from_str(&stderr).context("stderr should be valid JSON")?;
    let message = value
        .get("diagnostics")
        .and_then(Value::as_array)
        .and_then(|diagnostics| diagnostics.first())
        .and_then(|diagnostic| diagnostic.get("message"))
        .and_then(Value::as_str)
        .context("first diagnostic should include a message")?;
    ensure!(
        message.contains("invalid type") && message.contains("expected usize"),
        "merge failure should describe the config type mismatch: {message}"
    );
    Ok(())
}

#[test]
fn diag_json_success_discards_child_stderr() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let manifest_path = temp.path().join("Netsukefile");
    fs::copy("tests/data/minimal.yml", &manifest_path)
        .with_context(|| format!("copy manifest to {}", manifest_path.display()))?;

    let ninja_temp = tempdir().context("create fake ninja dir")?;
    let ninja_path = if cfg!(windows) {
        ninja_temp.path().join("fake-ninja.cmd")
    } else {
        ninja_temp.path().join("fake-ninja")
    };
    write_fake_ninja_script(
        &ninja_path,
        &["digraph G { hello -> world; }"],
        Some("NINJA_STDERR_MARKER"),
    )?;
    let env = SystemEnv::new();
    let _guard = override_ninja_env(&env, &ninja_path);

    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("--diag-json")
        .arg("graph")
        .output()
        .context("run netsuke graph with fake ninja")?;

    ensure!(output.status.success(), "expected graph command success");
    ensure!(
        output.stderr.is_empty(),
        "stderr should stay empty in JSON mode even when ninja writes to stderr"
    );
    let stdout = String::from_utf8(output.stdout).context("stdout should be valid UTF-8")?;
    ensure!(
        stdout.contains("digraph G"),
        "stdout should keep graph output"
    );
    ensure!(
        !stdout.contains("NINJA_STDERR_MARKER"),
        "child stderr should not leak into stdout"
    );
    Ok(())
}
