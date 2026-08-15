//! Integration tests for the in-process `netsuke help targets` subcommand.
//!
//! Shared fixtures and rejection scenarios remain here, while catalogue
//! rendering scenarios live in the cohesive [`catalogue`] child module.

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use netsuke::cli::{Cli, Commands, HelpArgs, HelpTopic};
use netsuke::output_prefs;
use netsuke::runner::run;
use rstest::{fixture, rstest};
use test_support::{fluent::normalize_fluent_isolates, localizer_test_lock, set_en_localizer};

#[path = "runner_help_targets_tests/catalogue.rs"]
mod catalogue;
mod fixtures;

use fixtures::create_test_manifest;

/// Write a manifest with actions, targets, defaults, and one entry whose
/// description is missing, so both catalogue sections are exercised.
fn write_help_targets_manifest(temp: &tempfile::TempDir) -> Result<Utf8PathBuf> {
    let temp_path = Utf8Path::from_path(temp.path()).context("temporary path should be UTF-8")?;
    let manifest_path = temp_path.join("Netsukefile");
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open help-targets fixture directory")?;
    workspace
        .write(
            "Netsukefile",
            r#"netsuke_version: "1.0.0"
actions:
  - name: lint
    description: Run rustdoc, Clippy, and Whitaker
    command: touch lint-ran
  - name: test
    description: Run unit, behavioural, UI, and documentation tests
    command: touch test-ran
targets:
  - name: target/release/catnap
    description: Build the optimized release binary
    command: touch release-ran
  - name: plain
    command: touch plain-ran
defaults:
  - lint
  - test
"#,
        )
        .with_context(|| format!("write manifest to {}", manifest_path.as_str()))?;
    Ok(manifest_path)
}

#[fixture]
fn help_targets_manifest() -> Result<(tempfile::TempDir, Utf8PathBuf)> {
    let temp = tempfile::tempdir().context("create help-targets fixture directory")?;
    let manifest_path = write_help_targets_manifest(&temp)?;
    Ok((temp, manifest_path))
}

fn run_help_targets(cli: &Cli) -> Result<()> {
    let _lock = localizer_test_lock().map_err(|e| anyhow::anyhow!("{e}"))?;
    let _guard = set_en_localizer();
    run(cli, output_prefs::resolve(None)).context("running help targets subcommand")
}

fn assert_help_targets_rejects_manifest(
    fixture_name: &str,
    manifest: &[u8],
    expected_error: &str,
) -> Result<()> {
    let temp =
        tempfile::tempdir().with_context(|| format!("create {fixture_name} fixture directory"))?;
    let temp_path = Utf8Path::from_path(temp.path())
        .with_context(|| format!("{fixture_name} temporary path should be UTF-8"))?;
    let manifest_path = temp_path.join("Netsukefile");
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .with_context(|| format!("open {fixture_name} fixture directory"))?;
    workspace
        .write("Netsukefile", manifest)
        .with_context(|| format!("write {fixture_name} manifest"))?;
    let cli = Cli {
        file: manifest_path.into_std_path_buf(),
        command: Some(Commands::Help(HelpArgs {
            topic: Some(HelpTopic::Targets),
        })),
        ..Cli::default()
    };
    let Err(error) = run_help_targets(&cli) else {
        anyhow::bail!("{fixture_name} manifest should fail help targets");
    };
    ensure!(
        error
            .chain()
            .any(|cause| normalize_fluent_isolates(&cause.to_string()).contains(expected_error)),
        "error should contain {expected_error:?}: {error:?}"
    );
    Ok(())
}

#[rstest]
fn help_targets_with_invalid_manifest_reports_error() -> Result<()> {
    let temp = tempfile::tempdir().context("temp dir")?;
    let temp_path = Utf8Path::from_path(temp.path()).context("temporary path should be UTF-8")?;
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open invalid-manifest fixture directory")?;
    let data = Dir::open_ambient_dir("tests/data", ambient_authority())
        .context("open invalid manifest fixture directory")?;
    let manifest_path = temp_path.join("Netsukefile");
    data.copy("invalid_version.yml", &workspace, "Netsukefile")
        .with_context(|| format!("copy invalid manifest to {}", manifest_path.as_str()))?;
    let cli = Cli {
        file: manifest_path.into_std_path_buf(),
        command: Some(Commands::Help(HelpArgs {
            topic: Some(HelpTopic::Targets),
        })),
        ..Cli::default()
    };
    let error = run_help_targets(&cli).expect_err("invalid manifest should fail help targets");
    ensure!(
        error
            .chain()
            .any(|cause| cause.to_string().contains("Manifest parse failed.")),
        "error should identify the manifest parsing failure: {error:?}"
    );
    Ok(())
}

#[test]
fn help_targets_rejects_valid_manifest_with_missing_rule() -> Result<()> {
    assert_help_targets_rejects_manifest(
        "missing-rule",
        b"netsuke_version: \"1.0.0\"\ntargets:\n  - name: out/app\n    rule: missing\n",
        "was not found",
    )
}

#[test]
fn help_targets_rejects_unknown_manifest_default() -> Result<()> {
    assert_help_targets_rejects_manifest(
        "unknown-default",
        b"netsuke_version: \"1.0.0\"\nactions:\n  - name: lint\n    command: cargo clippy\ntargets: []\ndefaults:\n  - missing\n",
        "default 'missing'",
    )
}

#[test]
fn help_targets_escapes_manifest_defaults_in_diagnostics() -> Result<()> {
    assert_help_targets_rejects_manifest(
        "unsafe-default",
        b"netsuke_version: \"1.0.0\"\nactions:\n  - name: lint\n    command: cargo clippy\ntargets: []\ndefaults:\n  - \"bad\\nINJECTED\"\n",
        r"default 'bad\nINJECTED'",
    )
}

#[test]
fn help_targets_does_not_emit_raw_manifest_controls_in_diagnostics() -> Result<()> {
    let temp = tempfile::tempdir().context("create unsafe-default diagnostic fixture directory")?;
    let temp_path = Utf8Path::from_path(temp.path())
        .context("unsafe-default diagnostic temporary path should be UTF-8")?;
    let manifest_path = temp_path.join("Netsukefile");
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open unsafe-default diagnostic fixture directory")?;
    workspace
        .write(
            "Netsukefile",
            b"netsuke_version: \"1.0.0\"\nactions:\n  - name: lint\n    command: cargo clippy\ntargets: []\ndefaults:\n  - \"bad\\nINJECTED\"\n",
        )
        .context("write unsafe-default diagnostic manifest")?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .arg("--file")
        .arg(&manifest_path)
        .arg("help")
        .arg("targets")
        .output()
        .context("run help targets against unsafe default")?;
    ensure!(
        !output.status.success(),
        "unsafe default should make help targets fail validation"
    );
    let stderr = normalize_fluent_isolates(&String::from_utf8_lossy(&output.stderr));
    ensure!(
        stderr.contains(r"default 'bad\nINJECTED'"),
        "diagnostic should show escaped manifest controls: {stderr}"
    );
    ensure!(
        !stderr.contains("default 'bad\nINJECTED'"),
        "diagnostic must not emit a raw manifest newline: {stderr}"
    );
    Ok(())
}

#[rstest]
fn plain_help_matches_minimal_workspace() -> Result<()> {
    let (temp, manifest_path) = create_test_manifest()?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("--file")
        .arg(manifest_path)
        .arg("help")
        .output()
        .context("run plain help against minimal workspace")?;
    ensure!(
        output.status.success(),
        "plain help should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("Usage: netsuke") && stdout.contains("Commands:"),
        "plain help should render root command text: {stdout}"
    );
    Ok(())
}
