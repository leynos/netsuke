//! Integration tests verifying that log output is written to stderr.
//!
//! These tests exercise the production logging path by invoking the compiled
//! binary and asserting log messages appear on stderr rather than stdout.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
#[cfg(unix)]
use cap_std::fs::PermissionsExt;
use cap_std::{ambient_authority, fs_utf8::Dir};
use netsuke::runner::NINJA_ENV;
use predicates::prelude::*;
use proptest::prelude::*;
use rstest::{fixture, rstest};
use serde_json::Value;
use std::fs;
use std::path::Path;
use tempfile::{TempDir, tempdir};

#[cfg(unix)]
fn make_script_executable(dir: &Dir, path: &Utf8Path) -> Result<()> {
    let mut permissions = dir
        .metadata(path)
        .with_context(|| format!("read metadata for {path}"))?
        .permissions();
    permissions.set_mode(0o755);
    dir.set_permissions(path, permissions)
        .with_context(|| format!("set executable bit for {path}"))?;
    Ok(())
}

#[cfg(not(unix))]
fn make_script_executable(_dir: &Dir, _path: &Utf8Path) -> Result<()> {
    Ok(())
}

#[fixture]
fn temp_with_minimal_manifest() -> Result<TempDir> {
    let temp = tempdir().context("create temp dir")?;
    let workspace_path = Utf8Path::from_path(temp.path()).context("temp dir path is not UTF-8")?;
    let workspace = Dir::open_ambient_dir(workspace_path, ambient_authority())
        .context("open temporary workspace")?;
    let repository = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open repository root")?;
    repository
        .copy("tests/data/minimal.yml", &workspace, "Netsukefile")
        .context("copy minimal manifest to temporary workspace")?;
    Ok(temp)
}

fn write_fake_ninja_script(
    dir: &Dir,
    path: &Utf8Path,
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

    dir.write(path, script)
        .with_context(|| format!("write fake ninja script {path}"))?;
    make_script_executable(dir, path)
}
fn fake_ninja_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.cmd")
    } else {
        stem.to_owned()
    }
}

fn path_containing(dir: &Path) -> Result<std::ffi::OsString> {
    std::env::join_paths([dir]).context("build PATH containing fake ninja")
}

fn run_verbose_build_with_ninja_env(
    current_dir: &Path,
    path_env: std::ffi::OsString,
    ninja_env: Option<&Path>,
) -> Result<String> {
    let mut command = assert_cmd::cargo::cargo_bin_cmd!("netsuke");
    command
        .current_dir(current_dir)
        .env("PATH", path_env)
        .env_remove(NINJA_ENV)
        .arg("--verbose")
        .arg("build");
    if let Some(ninja) = ninja_env {
        command.env(NINJA_ENV, ninja);
    }

    let output = command.output().context("run verbose netsuke build")?;
    ensure!(output.status.success(), "expected verbose build to succeed");
    String::from_utf8(output.stderr).context("stderr should be valid UTF-8")
}

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

#[rstest]
fn diag_json_success_keeps_stdout_artefact_and_stderr_empty(
    temp_with_minimal_manifest: Result<TempDir>,
) -> Result<()> {
    let temp = temp_with_minimal_manifest?;

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

fn run_verbose_build_and_assert_ninja_log(
    temp_with_minimal_manifest: Result<TempDir>,
    ninja_stem: &str,
    use_env_override: bool,
) -> Result<()> {
    let temp = temp_with_minimal_manifest?;
    let description = if use_env_override {
        "override build should log the resolved ninja program"
    } else {
        "default build should log the fallback ninja program"
    };
    run_verbose_build_with_fake_ninja_and_assert_log(
        temp.path(),
        ninja_stem,
        use_env_override,
        description,
    )
}

fn run_verbose_build_with_fake_ninja_and_assert_log(
    workspace: &Path,
    ninja_stem: &str,
    use_env_override: bool,
    description: &str,
) -> Result<()> {
    let ninja_temp = tempdir().context("create fake ninja dir")?;
    let ninja_dir_path =
        Utf8Path::from_path(ninja_temp.path()).context("fake ninja directory path is not UTF-8")?;
    let ninja_dir = Dir::open_ambient_dir(ninja_dir_path, ambient_authority())
        .context("open fake ninja directory")?;
    let ninja_name = fake_ninja_name(ninja_stem);
    write_fake_ninja_script(&ninja_dir, Utf8Path::new(&ninja_name), &[], None)?;
    let ninja_path = ninja_temp.path().join(&ninja_name);

    let ninja_env = use_env_override.then_some(ninja_path.as_path());
    let stderr = run_verbose_build_with_ninja_env(
        workspace,
        path_containing(ninja_temp.path())?,
        ninja_env,
    )?;

    let expected = if use_env_override {
        format!("Executing command: {} ", ninja_path.display())
    } else {
        format!("Executing command: {ninja_stem} ")
    };
    ensure!(stderr.contains(&expected), "{description}, got:\n{stderr}");
    Ok(())
}
/// Runs a verbose build with `NETSUKE_NINJA` set to a fake executable whose
/// stem is `stem` and asserts that the resulting log contains
/// `Executing command: <full_path> `.
fn assert_verbose_build_logs_ninja_override(stem: &str) -> Result<()> {
    let temp = temp_with_minimal_manifest()?;

    run_verbose_build_with_fake_ninja_and_assert_log(
        temp.path(),
        stem,
        true,
        &format!("verbose log must contain the resolved ninja path for override stem {stem:?}"),
    )
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 16, ..ProptestConfig::default() })]
    #[test]
    fn verbose_build_logs_resolved_ninja_program_for_any_valid_override(
        stem in "[a-z][a-z0-9_-]{0,15}",
    ) {
        assert_verbose_build_logs_ninja_override(&stem)
            .map_err(|e| TestCaseError::fail(e.to_string()))?;
    }
}

#[rstest]
fn verbose_build_logs_default_ninja_command(
    temp_with_minimal_manifest: Result<TempDir>,
) -> Result<()> {
    run_verbose_build_and_assert_ninja_log(temp_with_minimal_manifest, "ninja", false)
}

#[rstest]
fn verbose_build_logs_ninja_env_override(
    temp_with_minimal_manifest: Result<TempDir>,
) -> Result<()> {
    run_verbose_build_and_assert_ninja_log(temp_with_minimal_manifest, "custom-ninja", true)
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
fn output_format_json_formats_config_load_failures_as_json() -> Result<()> {
    let temp = tempdir().context("create temp dir")?;
    let config_path = temp.path().join("broken.toml");
    fs::write(&config_path, "theme = \"ascii\n")
        .with_context(|| format!("write malformed config {}", config_path.display()))?;

    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .args([
            "--config",
            &config_path.to_string_lossy(),
            "--output-format",
            "json",
        ])
        .output()
        .context("run netsuke with malformed config and JSON output format")?;

    ensure!(!output.status.success(), "expected config load failure");
    ensure!(
        output.stdout.is_empty(),
        "stdout should remain empty on config load failure"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be valid UTF-8")?;
    let value: Value = serde_json::from_str(&stderr).context("stderr should be valid JSON")?;
    ensure!(
        value.get("diagnostics").and_then(Value::as_array).is_some(),
        "JSON diagnostics should include a diagnostics array: {value:?}"
    );
    Ok(())
}

#[rstest]
fn diag_json_success_graph_keeps_clean_stderr(
    temp_with_minimal_manifest: Result<TempDir>,
) -> Result<()> {
    let temp = temp_with_minimal_manifest?;

    // `graph` renders in-process and never spawns Ninja, so no child stderr
    // is produced. Verify `--diag-json graph` still emits the DOT graph on
    // stdout and keeps stderr empty.
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("--diag-json")
        .arg("graph")
        .output()
        .context("run netsuke graph")?;

    ensure!(output.status.success(), "expected graph command success");
    ensure!(
        output.stderr.is_empty(),
        "stderr should stay empty in JSON mode"
    );
    let stdout = String::from_utf8(output.stdout).context("stdout should be valid UTF-8")?;
    ensure!(
        stdout.contains("digraph netsuke"),
        "stdout should carry the DOT graph; got: {stdout}"
    );
    Ok(())
}
