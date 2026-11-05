//! End-to-end CLI coverage for generated Ninja output.

use anyhow::{Context, Result, ensure};
use assert_cmd::Command;
use predicates::prelude::*;
use rstest::rstest;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{TempDir, tempdir};

fn setup_simple_workspace(context: &str) -> Result<TempDir> {
    let temp = tempdir().with_context(|| format!("create temp dir for {context}"))?;
    let netsukefile = temp.path().join("Netsukefile");
    fs::copy("tests/data/minimal.yml", &netsukefile)
        .with_context(|| format!("copy manifest to {} for {context}", netsukefile.display()))?;
    Ok(temp)
}

fn setup_workspace_with_subdir(context: &str) -> Result<(TempDir, PathBuf)> {
    let temp = tempdir().with_context(|| format!("create temp dir for {context}"))?;
    let workdir = temp.path().join("work");
    fs::create_dir_all(&workdir).with_context(|| format!("create work directory for {context}"))?;
    fs::copy("tests/data/minimal.yml", workdir.join("Netsukefile"))
        .with_context(|| format!("copy manifest for {context}"))?;
    Ok((temp, workdir))
}

fn create_netsuke_command(current_dir: &Path) -> Command {
    let mut command = assert_cmd::cargo::cargo_bin_cmd!("netsuke");
    command.current_dir(current_dir);
    command
}

#[cfg(unix)]
#[rstest]
fn build_rejects_command_injecting_glob_filename() -> Result<()> {
    let temp = tempdir().context("create command-injection test workspace")?;
    let files = temp.path().join("files");
    fs::create_dir(&files).context("create glob input directory")?;
    fs::write(
        files.join("a.txt; touch PWNED; #.txt"),
        "attacker-controlled filename",
    )
    .context("write malicious glob input")?;
    let pattern = files.join("*.txt");
    let manifest = format!(
        concat!(
            "netsuke_version: '1.0.0'\n",
            "targets:\n",
            "  - foreach: glob({pattern:?})\n",
            "    name: output-{{{{ index }}}}.txt\n",
            "    command: echo {{{{ item }}}}\n"
        ),
        pattern = pattern.display()
    );
    fs::write(temp.path().join("Netsukefile"), manifest)
        .context("write command-injection test manifest")?;

    let output = create_netsuke_command(temp.path())
        .args(["--json", "build"])
        .output()
        .context("run Netsuke build with JSON diagnostics")?;
    ensure!(
        !output.status.success(),
        "the unsafe filename must stop the Netsuke build"
    );
    let stderr = String::from_utf8(output.stderr).context("build stderr should be valid UTF-8")?;
    ensure!(
        stderr.contains("characters that require shell quoting"),
        "the failed build must report the shell-quoting rejection: {stderr}"
    );
    ensure!(
        !temp.path().join("PWNED").exists(),
        "the attacker-controlled filename must not create PWNED"
    );
    Ok(())
}

fn assert_generate_streams_to_stdout(
    current_dir: &Path,
    args: &[&str],
    command_description: &str,
) -> Result<()> {
    let output = create_netsuke_command(current_dir)
        .args(args)
        .output()
        .with_context(|| format!("run {command_description}"))?;
    ensure!(
        output.status.success(),
        "{command_description} should succeed"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("rule ") && stdout.contains("build "),
        "{command_description} should print Ninja content, got: {stdout}"
    );
    Ok(())
}

#[rstest]
fn generate_writes_file() -> Result<()> {
    let temp = setup_simple_workspace("generate file test")?;
    let output = temp.path().join("standalone.ninja");

    create_netsuke_command(temp.path())
        .args(["generate", "--output"])
        .arg(&output)
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    ensure!(output.exists(), "generate should create the output file");
    let contents = fs::read_to_string(&output)
        .with_context(|| format!("read generated file {}", output.display()))?;
    ensure!(
        contents.contains("rule ") && contents.contains("build "),
        "generated file should contain Ninja content, got: {contents}"
    );
    Ok(())
}

#[rstest]
fn generate_rejects_unsupported_schema_before_creating_output() -> Result<()> {
    let temp = tempdir().context("create schema-version generation workspace")?;
    let manifest_path = temp.path().join("Netsukefile");
    let output_path = temp.path().join("out.ninja");
    fs::write(
        &manifest_path,
        concat!(
            "netsuke_version: '1.1.0'\n",
            "targets:\n",
            "  - name: hello\n",
            "    command: echo hello\n",
        ),
    )
    .context("write unsupported schema-version manifest")?;

    let unsupported = create_netsuke_command(temp.path())
        .args(["--json", "generate", "--output", "out.ninja"])
        .output()
        .context("run generate with an unsupported schema version")?;
    ensure!(
        !unsupported.status.success(),
        "generate must reject schema version 1.1.0"
    );
    ensure!(
        !output_path.exists(),
        "generate must not create output for a rejected manifest"
    );
    let stderr =
        String::from_utf8(unsupported.stderr).context("JSON diagnostics should be valid UTF-8")?;
    let diagnostics: serde_json::Value =
        serde_json::from_str(&stderr).context("parse generate diagnostics JSON")?;
    let diagnostic = diagnostics
        .pointer("/diagnostics/0")
        .context("generate should report a diagnostic")?;
    ensure!(
        diagnostic.get("code").and_then(serde_json::Value::as_str)
            == Some("netsuke::manifest::parse"),
        "top-level parse diagnostic should keep its stable code: {diagnostic}"
    );
    let causes = diagnostic
        .get("causes")
        .and_then(serde_json::Value::as_array)
        .context("parse diagnostic should include causes")?;
    ensure!(
        causes.iter().any(|cause| cause
            .as_str()
            .is_some_and(|text| text.contains("netsuke::manifest::schema_version"))),
        "cause should identify schema-version rejection: {causes:?}"
    );

    fs::write(
        &manifest_path,
        concat!(
            "netsuke_version: '1.0.0'\n",
            "targets:\n",
            "  - name: hello\n",
            "    command: echo hello\n",
        ),
    )
    .context("write supported schema-version manifest")?;
    create_netsuke_command(temp.path())
        .args(["generate", "--output", "out.ninja"])
        .assert()
        .success();
    ensure!(
        output_path.exists(),
        "the paired supported manifest should generate output"
    );
    Ok(())
}

#[cfg(feature = "lint")]
#[rstest]
fn check_rejects_unsupported_schema_and_accepts_supported_schema() -> Result<()> {
    let temp = tempdir().context("create schema-version check workspace")?;
    let manifest_path = temp.path().join("Netsukefile");
    fs::write(
        &manifest_path,
        concat!(
            "netsuke_version: '1.1.0'\n",
            "targets:\n",
            "  - name: hello\n",
            "    command: echo hello\n",
        ),
    )
    .context("write unsupported schema-version manifest")?;
    let unsupported = create_netsuke_command(temp.path())
        .args(["--json", "check"])
        .output()
        .context("run check with an unsupported schema version")?;
    ensure!(
        !unsupported.status.success(),
        "check must reject schema version 1.1.0"
    );
    let stderr =
        String::from_utf8(unsupported.stderr).context("check diagnostics should be valid UTF-8")?;
    let diagnostics: serde_json::Value =
        serde_json::from_str(&stderr).context("parse check diagnostics JSON")?;
    let diagnostic = diagnostics
        .pointer("/diagnostics/0")
        .context("check should report a diagnostic")?;
    ensure!(
        diagnostic.get("code").and_then(serde_json::Value::as_str)
            == Some("netsuke::manifest::parse"),
        "check should preserve the parse diagnostic code: {diagnostic}"
    );
    let causes = diagnostic
        .get("causes")
        .and_then(serde_json::Value::as_array)
        .context("check diagnostic should include causes")?;
    ensure!(
        causes.iter().any(|cause| cause
            .as_str()
            .is_some_and(|text| text.contains("netsuke::manifest::schema_version"))),
        "check should expose the schema diagnostic code: {causes:?}"
    );

    fs::write(
        &manifest_path,
        concat!(
            "netsuke_version: '1.0.0'\n",
            "targets:\n",
            "  - name: hello\n",
            "    command: echo hello\n",
        ),
    )
    .context("write supported schema-version manifest")?;
    create_netsuke_command(temp.path())
        .args(["check"])
        .assert()
        .success();
    Ok(())
}

#[rstest]
fn generate_streams_to_stdout_by_default() -> Result<()> {
    let temp = setup_simple_workspace("generate stdout test")?;
    assert_generate_streams_to_stdout(temp.path(), &["generate"], "netsuke generate")
}

#[rstest]
fn generate_resolves_output_relative_to_directory() -> Result<()> {
    let (temp, workdir) = setup_workspace_with_subdir("generate -C test")?;

    create_netsuke_command(temp.path())
        .args(["-C", "work", "generate", "--output", "out.ninja"])
        .assert()
        .success();

    ensure!(
        workdir.join("out.ninja").exists(),
        "generate output should be written relative to -C directory"
    );
    ensure!(
        !temp.path().join("out.ninja").exists(),
        "generate output should not be written outside -C directory"
    );
    Ok(())
}

#[rstest]
fn generate_streams_to_stdout_with_directory() -> Result<()> {
    let (temp, _workdir) = setup_workspace_with_subdir("generate stdout -C test")?;
    assert_generate_streams_to_stdout(
        temp.path(),
        &["-C", "work", "generate"],
        "netsuke -C work generate",
    )
}
