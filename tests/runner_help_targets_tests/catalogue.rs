//! Catalogue-rendering scenarios for `netsuke help targets`.

use super::*;
use netsuke::{ir::BuildGraph, manifest, ninja_gen};
use serde_json::Value;

fn assert_fixture_recipes_not_run(workspace: &Dir) -> Result<()> {
    for output in ["lint-ran", "test-ran", "release-ran", "plain-ran"] {
        ensure!(
            workspace.open(output).is_err(),
            "help targets must not execute the recipe that creates {output}"
        );
    }
    ensure!(
        workspace.open(".netsuke").is_err(),
        "help targets must not create a build-output directory"
    );
    Ok(())
}

fn assert_foreach_help_catalogue(manifest_path: &Utf8Path) -> Result<()> {
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .arg("--file")
        .arg(manifest_path)
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
        "check-unit",
        "Run unit",
        "check-integration",
        "Run integration",
    ] {
        ensure!(
            stdout.contains(expected),
            "catalogue should render foreach description {expected:?}: {stdout}"
        );
    }
    Ok(())
}

#[rstest]
fn help_targets_prints_actions_and_targets(
    #[from(help_targets_manifest)] fixture: Result<(tempfile::TempDir, Utf8PathBuf)>,
) -> Result<()> {
    let (_temp, manifest_path) = fixture?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .env("NETSUKE_NINJA", "/definitely-not-a-ninja-binary")
        .arg("--file")
        .arg(&manifest_path)
        .arg("help")
        .arg("targets")
        .output()
        .context("run netsuke help targets")?;
    ensure!(
        output.status.success(),
        "help targets should succeed without starting Ninja; stderr: {}",
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
fn help_targets_accessible_output_marks_defaults_without_recipes(
    #[from(help_targets_manifest)] fixture: Result<(tempfile::TempDir, Utf8PathBuf)>,
) -> Result<()> {
    let (temp, manifest_path) = fixture?;
    let temp_path = Utf8Path::from_path(temp.path()).context("temporary path should be UTF-8")?;
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open accessible help-targets fixture directory")?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp_path)
        .arg("--accessibility")
        .arg("on")
        .arg("--file")
        .arg(&manifest_path)
        .arg("help")
        .arg("targets")
        .output()
        .context("run accessible netsuke help targets")?;
    ensure!(
        output.status.success(),
        "accessible help targets should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("[* default]"),
        "accessible catalogue should use the ASCII default marker: {stdout}"
    );
    assert_fixture_recipes_not_run(&workspace)
}

#[rstest]
fn help_targets_localizes_output_without_recipes(
    #[from(help_targets_manifest)] fixture: Result<(tempfile::TempDir, Utf8PathBuf)>,
) -> Result<()> {
    let (temp, manifest_path) = fixture?;
    let temp_path = Utf8Path::from_path(temp.path()).context("temporary path should be UTF-8")?;
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open localized help-targets fixture directory")?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp_path)
        .arg("--locale")
        .arg("es-ES")
        .arg("--emoji")
        .arg("always")
        .arg("--file")
        .arg(&manifest_path)
        .arg("help")
        .arg("targets")
        .output()
        .context("run localized netsuke help targets")?;
    ensure!(
        output.status.success(),
        "localized help targets should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for expected in ["Acciones:", "Objetivos:", "[★ predeterminado]"] {
        ensure!(
            stdout.contains(expected),
            "localized catalogue should contain {expected:?}: {stdout}"
        );
    }
    assert_fixture_recipes_not_run(&workspace)
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
actions:
  - name: check-{{ item }}
    description: Run {{ item }}
    command: touch action-{{ item }}
    foreach:
      - unit
      - integration
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
    assert_foreach_help_catalogue(&manifest_path)?;
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
    ensure!(
        workspace.open("action-unit").is_err() && workspace.open("action-integration").is_err(),
        "help targets must not execute action recipes"
    );
    Ok(())
}

#[rstest]
fn help_targets_rejects_impure_description_without_creating_outputs() -> Result<()> {
    let temp = tempfile::tempdir().context("create impure-query help-targets workspace")?;
    let temp_path = Utf8Path::from_path(temp.path()).context("temporary path should be UTF-8")?;
    let manifest_path = temp_path.join("Netsukefile");
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open impure-query help-targets fixture directory")?;
    workspace
        .write(
            "Netsukefile",
            r#"netsuke_version: "1.0.0"
actions:
  - name: query-environment
    description: "{{ env('PATH') }}"
    command: touch lint-ran
targets:
  - name: generated-file
    description: Generate the file
    command: touch release-ran
defaults:
  - query-environment
"#,
        )
        .context("write impure-query manifest")?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp_path)
        .arg("--file")
        .arg(&manifest_path)
        .arg("help")
        .arg("targets")
        .output()
        .context("run help targets against impure description")?;
    ensure!(
        !output.status.success(),
        "help targets must reject a manifest query that reads the environment"
    );
    ensure!(
        output.stdout.is_empty(),
        "failed help targets must not write a partial catalogue: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("Deserializing and rendering manifest values")
            && stderr.contains("Failed to load manifest")
            && !stderr.contains("Building and validating dependency graph"),
        "disabled helper should reject the manifest while descriptions render: {stderr}"
    );
    ensure!(
        workspace.open("lint-ran").is_err() && workspace.open("release-ran").is_err(),
        "rejected help targets must not execute manifest recipes"
    );
    ensure!(
        workspace.open(".netsuke").is_err(),
        "rejected help targets must not create a build-output directory"
    );
    Ok(())
}

#[rstest]
#[case("build", "Usage: build")]
#[case("clean", "Usage: clean")]
#[case("graph", "Usage: graph")]
#[case("generate", "Usage: generate")]
fn nested_help_topics_render_at_the_command_boundary(
    #[case] topic: &str,
    #[case] expected: &str,
) -> Result<()> {
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .arg("help")
        .arg(topic)
        .output()
        .with_context(|| format!("run help topic {topic}"))?;
    ensure!(
        output.status.success(),
        "help topic {topic} should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains(expected),
        "help topic {topic} should render its command text: {stdout}"
    );
    Ok(())
}
