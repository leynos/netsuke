//! Integration tests for the in-process `netsuke help targets` subcommand.
//!
//! The `help targets` subcommand loads, expands, renders, and validates the
//! selected manifest without invoking Ninja, then prints the target and action
//! catalogue. These tests verify the dispatch works without Ninja installed,
//! honours `--file` and `-C/--directory`, and emits the expected JSON envelope
//! in `--json` mode.

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use netsuke::output_prefs;
use netsuke::runner::run;
use netsuke::{
    cli::{Cli, Commands, HelpArgs, HelpTopic},
    ir::BuildGraph,
    manifest, ninja_gen,
};
use rstest::{fixture, rstest};
use serde_json::Value;
use test_support::{localizer_test_lock, set_en_localizer};

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
    command: cargo clippy --all-targets --all-features -- -D warnings
  - name: test
    description: Run unit, behavioural, UI, and documentation tests
    command: cargo test
targets:
  - name: target/release/catnap
    description: Build the optimized release binary
    command: cargo build --release
  - name: plain
    command: echo plain
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

#[rstest]
fn help_targets_prints_actions_and_targets(
    #[from(help_targets_manifest)] fixture: Result<(tempfile::TempDir, Utf8PathBuf)>,
) -> Result<()> {
    let (_temp, manifest_path) = fixture?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .arg("--file")
        .arg(&manifest_path)
        .arg("help")
        .arg("targets")
        .output()
        .context("run netsuke help targets")?;
    ensure!(
        output.status.success(),
        "help targets should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("Actions:") && stdout.contains("Targets:"),
        "catalogue should carry both sections: {stdout}"
    );
    ensure!(
        stdout.contains("Run rustdoc, Clippy, and Whitaker"),
        "description should be rendered: {stdout}"
    );
    ensure!(
        stdout.contains("plain")
            && !stdout
                .lines()
                .any(|line| line.contains("plain") && line.contains("Build the")),
        "an undocumented entry should still be listed without a description: {stdout}"
    );
    Ok(())
}

#[rstest]
fn help_targets_json_reports_command_identifier(
    #[from(help_targets_manifest)] fixture: Result<(tempfile::TempDir, Utf8PathBuf)>,
) -> Result<()> {
    let (temp, manifest_path) = fixture?;
    let temp_path = Utf8Path::from_path(temp.path()).context("temporary path should be UTF-8")?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp_path)
        .arg("--json")
        .arg("--file")
        .arg(&manifest_path)
        .arg("help")
        .arg("targets")
        .output()
        .context("run netsuke --json help targets")?;

    ensure!(
        output.status.success(),
        "help targets --json should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).context("stdout should be valid UTF-8")?;
    let result: Value =
        serde_json::from_str(&stdout).context("stdout should be one JSON document")?;
    ensure!(
        result.pointer("/result/command").and_then(Value::as_str) == Some("help-targets"),
        "JSON result should identify the help-targets command: {result}"
    );
    ensure!(
        result
            .pointer("/result/actions")
            .and_then(Value::as_array)
            .is_some_and(|actions| actions
                .iter()
                .any(|entry| { entry.pointer("/name").and_then(Value::as_str) == Some("lint") })),
        "JSON result should list the lint action: {result}"
    );
    ensure!(
        result
            .pointer("/result/targets")
            .and_then(Value::as_array)
            .is_some_and(|targets| targets.iter().any(|entry| {
                entry.pointer("/name").and_then(Value::as_str) == Some("target/release/catnap")
            })),
        "JSON result should list the release target: {result}"
    );
    Ok(())
}

#[rstest]
fn help_targets_honours_directory_flag(
    #[from(help_targets_manifest)] fixture: Result<(tempfile::TempDir, Utf8PathBuf)>,
) -> Result<()> {
    let (temp, _manifest_path) = fixture?;
    let temp_path = Utf8Path::from_path(temp.path()).context("temporary path should be UTF-8")?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .arg("-C")
        .arg(temp_path)
        .arg("help")
        .arg("targets")
        .output()
        .context("run netsuke -C <dir> help targets")?;
    ensure!(
        output.status.success(),
        "help targets with -C should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("Actions:") && stdout.contains("Targets:"),
        "catalogue should carry both sections: {stdout}"
    );
    ensure!(
        stdout.contains("lint") && stdout.contains("target/release/catnap"),
        "catalogue should list the fixture names: {stdout}"
    );
    Ok(())
}

#[rstest]
fn help_targets_renders_foreach_descriptions_without_changing_rule_progress() -> Result<()> {
    let temp = tempfile::tempdir().context("create foreach help-targets workspace")?;
    let temp_path = Utf8Path::from_path(temp.path()).context("temporary path should be UTF-8")?;
    let manifest_path = temp_path.join("Netsukefile");
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open foreach help-targets fixture directory")?;
    workspace
        .write(
            "Netsukefile",
            r#"netsuke_version: "1.0.0"
rules:
  - name: render-report
    description: Render reports through the shared rule
    command: touch $out
targets:
  - name: report-{{ item }}
    description: Build the {{ item }} report
    rule: render-report
    foreach:
      - weekly
      - monthly
"#,
        )
        .context("write foreach manifest")?;

    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .arg("--file")
        .arg(&manifest_path)
        .arg("help")
        .arg("targets")
        .output()
        .context("run help targets against foreach manifest")?;
    ensure!(
        output.status.success(),
        "help targets should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "report-weekly",
        "Build the weekly report",
        "report-monthly",
        "Build the monthly report",
    ] {
        ensure!(
            stdout.contains(expected),
            "catalogue should render foreach description {expected:?}: {stdout}"
        );
    }

    let manifest = manifest::from_path(&manifest_path)?;
    let graph = BuildGraph::from_manifest(&manifest).context("generate foreach graph")?;
    let ninja = ninja_gen::generate(&graph).context("generate foreach Ninja manifest")?;
    ensure!(
        ninja.contains("description = Render reports through the shared rule"),
        "Ninja should retain the rule progress description: {ninja}"
    );
    ensure!(
        !ninja.contains("Build the weekly report") && !ninja.contains("Build the monthly report"),
        "target discovery descriptions must not replace Ninja progress: {ninja}"
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
    let temp = tempfile::tempdir().context("create missing-rule fixture directory")?;
    let temp_path = Utf8Path::from_path(temp.path()).context("temporary path should be UTF-8")?;
    let manifest_path = temp_path.join("Netsukefile");
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open missing-rule fixture directory")?;
    workspace
        .write(
            "Netsukefile",
            b"netsuke_version: \"1.0.0\"\ntargets:\n  - name: out/app\n    rule: missing\n",
        )
        .context("write missing-rule manifest")?;
    let cli = Cli {
        file: manifest_path.into_std_path_buf(),
        command: Some(Commands::Help(HelpArgs {
            topic: Some(HelpTopic::Targets),
        })),
        ..Cli::default()
    };

    let error = run_help_targets(&cli).expect_err("missing manifest rule should fail help targets");
    ensure!(
        error
            .chain()
            .any(|cause| cause.to_string().contains("was not found")),
        "IR validation should report the missing rule: {error:?}"
    );
    Ok(())
}

#[test]
fn help_targets_rejects_unknown_manifest_default() -> Result<()> {
    let (temp, manifest_path) = help_targets_manifest()?;
    let temp_path = Utf8Path::from_path(temp.path()).context("temporary path should be UTF-8")?;
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open unknown-default fixture directory")?;
    workspace
        .write(
            "Netsukefile",
            b"netsuke_version: \"1.0.0\"\nactions:\n  - name: lint\n    command: cargo clippy\ntargets: []\ndefaults:\n  - missing\n",
        )
        .context("write unknown-default manifest")?;
    let cli = Cli {
        file: manifest_path.into_std_path_buf(),
        command: Some(Commands::Help(HelpArgs {
            topic: Some(HelpTopic::Targets),
        })),
        ..Cli::default()
    };
    let error = run_help_targets(&cli).expect_err("unknown manifest default should fail");
    ensure!(
        error
            .chain()
            .any(|cause| cause.to_string().contains("default 'missing'")),
        "error should identify the unknown default: {error:?}"
    );
    Ok(())
}

#[rstest]
fn plain_help_matches_minimal_workspace() -> Result<()> {
    let (temp, manifest_path) = create_test_manifest()?;
    let cli = Cli {
        file: manifest_path,
        directory: Some(temp.path().to_path_buf()),
        command: Some(Commands::Help(HelpArgs { topic: None })),
        ..Cli::default()
    };
    run_help_targets(&cli)?;
    Ok(())
}
