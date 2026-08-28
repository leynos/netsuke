//! Discovery scenarios containing helpers unavailable to manifest queries.

use super::*;

/// Assert that every expected conditional catalogue row has its ASCII marker.
fn assert_conditional_catalogue_entries(stdout: &str) -> Result<()> {
    for (name, description) in [
        ("test", "Run tests with cargo-nextest"),
        ("test", "Run tests with Cargo"),
        (
            "target/test-result",
            "Build test results with cargo-nextest",
        ),
        ("target/test-result", "Build test results with Cargo"),
    ] {
        let entry = stdout
            .lines()
            .find(|line| line.contains(name) && line.contains(description))
            .with_context(|| format!("conditional catalogue should contain {name:?}: {stdout}"))?;
        ensure!(
            entry.contains("[? conditional]"),
            "conditional catalogue entry {name:?} should include its marker: {entry}"
        );
    }
    Ok(())
}

/// Assert that target discovery did not create any named workspace outputs.
fn assert_outputs_were_not_created(workspace: &Dir, outputs: &[&str]) -> Result<()> {
    for output in outputs {
        ensure!(
            workspace.open(output).is_err(),
            "help targets must not create {output}"
        );
    }
    Ok(())
}

#[rstest]
fn help_targets_skips_inline_build_only_helpers_in_recipes() -> Result<()> {
    let temp = tempfile::tempdir().context("create inline-helper help-targets workspace")?;
    let temp_path = Utf8Path::from_path(temp.path()).context("temporary path should be UTF-8")?;
    let manifest_path = temp_path.join("Netsukefile");
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open inline-helper fixture directory")?;
    workspace
        .write(
            "Netsukefile",
            r#"netsuke_version: "1.0.0"
actions:
  - name: test
    description: Run tests with cargo-nextest or Cargo
    command: >-
      RUSTFLAGS='-D warnings'
      cargo {% if command_available("cargo-nextest") %}nextest run{% else %}test{% endif %}
      --all-targets --all-features
targets: []
"#,
        )
        .context("write inline-helper manifest")?;

    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp_path)
        .arg("--file")
        .arg(&manifest_path)
        .arg("--emoji")
        .arg("never")
        .arg("help")
        .arg("targets")
        .output()
        .context("run help targets against inline helper")?;
    ensure!(
        output.status.success(),
        "help targets should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        String::from_utf8_lossy(&output.stdout).contains("Run tests with cargo-nextest or Cargo"),
        "catalogue should preserve the action description"
    );
    ensure!(
        workspace.open(".netsuke").is_err(),
        "help targets must not create a build-output directory"
    );
    Ok(())
}

/// List actions using build-only helpers as conditional entries.
#[rstest]
fn help_targets_lists_build_only_when_actions_as_conditional() -> Result<()> {
    let temp = tempfile::tempdir().context("create conditional help-targets workspace")?;
    let temp_path = Utf8Path::from_path(temp.path()).context("temporary path should be UTF-8")?;
    let manifest_path = temp_path.join("Netsukefile");
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open conditional fixture directory")?;
    workspace
        .write(
            "Netsukefile",
            r#"netsuke_version: "1.0.0"
actions:
  - name: preferred
    description: Run tests with cargo-nextest
    command: touch preferred-ran
    when: command_available("cargo-nextest")
  - name: fallback
    description: Run tests with Cargo
    command: touch fallback-ran
    when: not command_available("cargo-nextest")
targets: []
"#,
        )
        .context("write conditional manifest")?;

    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp_path)
        .arg("--file")
        .arg(&manifest_path)
        .arg("--emoji")
        .arg("never")
        .arg("help")
        .arg("targets")
        .output()
        .context("run help targets against conditional actions")?;
    ensure!(
        output.status.success(),
        "help targets should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for (name, description) in [
        ("preferred", "Run tests with cargo-nextest"),
        ("fallback", "Run tests with Cargo"),
    ] {
        let entry = stdout
            .lines()
            .find(|line| line.contains(name) && line.contains(description))
            .with_context(|| format!("conditional catalogue should contain {name:?}: {stdout}"))?;
        ensure!(
            entry.contains("[? conditional]"),
            "conditional catalogue entry {name:?} should include its marker: {entry}"
        );
    }
    for forbidden_output in ["preferred-ran", "fallback-ran", ".netsuke"] {
        ensure!(
            workspace.open(forbidden_output).is_err(),
            "help targets must not create {forbidden_output}"
        );
    }
    Ok(())
}

/// List same-name conditional alternatives in target help output.
#[rstest]
fn help_targets_lists_same_name_conditional_alternatives() -> Result<()> {
    let temp =
        tempfile::tempdir().context("create duplicate conditional help-targets workspace")?;
    let temp_path = Utf8Path::from_path(temp.path()).context("temporary path should be UTF-8")?;
    let manifest_path = temp_path.join("Netsukefile");
    let workspace = Dir::open_ambient_dir(temp_path, ambient_authority())
        .context("open duplicate conditional fixture directory")?;
    workspace
        .write(
            "Netsukefile",
            r#"netsuke_version: "1.0.0"
actions:
  - name: test
    description: Run tests with cargo-nextest
    command: touch preferred-action-ran
    when: command_available("cargo-nextest")
  - name: test
    description: Run tests with Cargo
    command: touch fallback-action-ran
    when: not command_available("cargo-nextest")
targets:
  - name: target/test-result
    description: Build test results with cargo-nextest
    command: touch preferred-target-ran
    when: command_available("cargo-nextest")
  - name: target/test-result
    description: Build test results with Cargo
    command: touch fallback-target-ran
    when: not command_available("cargo-nextest")
"#,
        )
        .context("write duplicate conditional manifest")?;

    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp_path)
        .arg("--file")
        .arg(&manifest_path)
        .arg("--emoji")
        .arg("never")
        .arg("help")
        .arg("targets")
        .output()
        .context("run help targets against duplicate conditional entries")?;
    ensure!(
        output.status.success(),
        "help targets should retain mutually exclusive alternatives; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_conditional_catalogue_entries(&stdout)?;
    assert_outputs_were_not_created(
        &workspace,
        &[
            "preferred-action-ran",
            "fallback-action-ran",
            "preferred-target-ran",
            "fallback-target-ran",
            ".netsuke",
        ],
    )?;
    Ok(())
}
