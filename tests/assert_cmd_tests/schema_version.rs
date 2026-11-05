//! End-to-end CLI coverage for the manifest schema-version gate.
//!
//! The cases live beside the general CLI tests so neither file dilutes the
//! other's function mix. Each drives the real binary, so the assertions bind
//! the process boundary rather than the loader alone.

use super::create_netsuke_command;
use anyhow::{Context, Result, ensure};
use rstest::{fixture, rstest};
use std::fs;
use std::path::Path;
use tempfile::{TempDir, tempdir};

/// Write the minimal hello manifest declaring `version` as its schema.
fn write_schema_manifest(directory: &Path, version: &str) -> Result<()> {
    fs::write(
        directory.join("Netsukefile"),
        format!(
            concat!(
                "netsuke_version: '{version}'\n",
                "targets:\n",
                "  - name: hello\n",
                "    command: echo hello\n",
            ),
            version = version
        ),
    )
    .context("write schema-version manifest")
}

/// Create a workspace whose manifest declares an unsupported schema version.
#[fixture]
fn unsupported_schema_workspace() -> Result<TempDir> {
    let temp = tempdir().context("create schema-version workspace")?;
    write_schema_manifest(temp.path(), "1.1.0")?;
    Ok(temp)
}

/// Assert a JSON-mode rejection carries the parse code and the schema cause.
fn assert_schema_rejection_json(stderr: &[u8], command: &str) -> Result<()> {
    let rendered = String::from_utf8_lossy(stderr);
    let diagnostics: serde_json::Value = serde_json::from_str(&rendered)
        .with_context(|| format!("parse {command} diagnostics JSON"))?;
    let diagnostic = diagnostics
        .pointer("/diagnostics/0")
        .with_context(|| format!("{command} should report a diagnostic"))?;
    ensure!(
        diagnostic.get("code").and_then(serde_json::Value::as_str)
            == Some("netsuke::manifest::parse"),
        "{command} should preserve the parse diagnostic code: {diagnostic}"
    );
    let causes = diagnostic
        .get("causes")
        .and_then(serde_json::Value::as_array)
        .with_context(|| format!("{command} diagnostic should include causes"))?;
    ensure!(
        causes.iter().any(|cause| cause
            .as_str()
            .is_some_and(|text| text.contains("netsuke::manifest::schema_version"))),
        "{command} should expose the schema diagnostic code: {causes:?}"
    );
    Ok(())
}

#[rstest]
fn generate_rejects_unsupported_schema_before_creating_output(
    unsupported_schema_workspace: Result<TempDir>,
) -> Result<()> {
    let temp = unsupported_schema_workspace?;
    let output_path = temp.path().join("out.ninja");

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
    assert_schema_rejection_json(&unsupported.stderr, "generate")?;

    write_schema_manifest(temp.path(), "1.0.0")?;
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

#[rstest]
fn human_output_renders_the_schema_version_rejection(
    unsupported_schema_workspace: Result<TempDir>,
) -> Result<()> {
    let temp = unsupported_schema_workspace?;

    let output = create_netsuke_command(temp.path())
        .args(["generate", "--output", "out.ninja"])
        .output()
        .context("run generate without JSON diagnostics")?;
    ensure!(
        !output.status.success(),
        "generate must reject schema version 1.1.0 on the human path too"
    );
    ensure!(
        !temp.path().join("out.ninja").exists(),
        "generate must not create output for a rejected manifest"
    );
    let stderr = String::from_utf8(output.stderr).context("human diagnostics should be UTF-8")?;
    ensure!(
        stderr.contains("netsuke::manifest::schema_version"),
        "human output must name the schema-version diagnostic code: {stderr}"
    );
    ensure!(
        stderr.contains("1.0"),
        "human output must name the schema this Netsuke supports: {stderr}"
    );
    Ok(())
}

#[cfg(feature = "lint")]
#[rstest]
fn check_rejects_unsupported_schema_and_accepts_supported_schema(
    unsupported_schema_workspace: Result<TempDir>,
) -> Result<()> {
    let temp = unsupported_schema_workspace?;
    let unsupported = create_netsuke_command(temp.path())
        .args(["--json", "check"])
        .output()
        .context("run check with an unsupported schema version")?;
    ensure!(
        !unsupported.status.success(),
        "check must reject schema version 1.1.0"
    );
    assert_schema_rejection_json(&unsupported.stderr, "check")?;

    write_schema_manifest(temp.path(), "1.0.0")?;
    create_netsuke_command(temp.path())
        .args(["check"])
        .assert()
        .success();
    Ok(())
}
