//! Discovery scenarios containing helpers unavailable to manifest queries.

use super::*;

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
    for expected in [
        "preferred",
        "Run tests with cargo-nextest",
        "fallback",
        "Run tests with Cargo",
        "[? conditional]",
    ] {
        ensure!(
            stdout.contains(expected),
            "conditional catalogue should contain {expected:?}: {stdout}"
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
