//! Exercise operator manifest budgets through the real command-line binary.

use anyhow::{Context, Result, ensure};
use assert_cmd::cargo::cargo_bin_cmd;
use rstest::rstest;
use tempfile::tempdir;
use test_support::{fluent::normalize_fluent_isolates, fs as test_fs};

#[rstest]
#[case::cli_flag_human(true, false)]
#[case::environment_human(false, false)]
#[case::cli_flag_json(true, true)]
#[case::environment_json(false, true)]
fn generate_enforces_operator_budget_before_writing_output(
    #[case] use_flag: bool,
    #[case] json: bool,
) -> Result<()> {
    let workspace = tempdir().context("create budget CLI workspace")?;
    test_fs::write(workspace.path().join("operator.toml"), "")?;
    test_fs::write(
        workspace.path().join("Netsukefile"),
        concat!(
            "netsuke_version: 1.0.0\n",
            "targets:\n",
            "  - name: limited\n",
            "    command: '{{ \"s3cr3t\" * 3 }}'\n",
        ),
    )?;
    let mut command = cargo_bin_cmd!("netsuke");
    command.current_dir(workspace.path()).env_clear().args([
        "--config",
        "operator.toml",
        "--locale",
        "en-US",
    ]);
    if use_flag {
        command.args(["--manifest-rendered-value-bytes", "16"]);
    } else {
        command.env("NETSUKE_MANIFEST_RENDERED_VALUE_BYTES", "16");
    }
    if json {
        command.arg("--json");
    }
    let output = command
        .args(["generate", "--output", "build.ninja"])
        .output()
        .context("run generate with a small operator budget")?;
    let stderr = normalize_fluent_isolates(&String::from_utf8_lossy(&output.stderr));
    ensure!(!output.status.success(), "over-budget generation must fail");
    if json {
        let document: serde_json::Value =
            serde_json::from_str(&stderr).context("budget diagnostic must be valid JSON")?;
        ensure!(
            document
                .pointer("/diagnostics/0/code")
                .and_then(serde_json::Value::as_str)
                == Some("netsuke::runner::manifest_budget_exceeded"),
            "budget diagnostic must have a stable code: {stderr}"
        );
    }
    ensure!(
        stderr.contains("Manifest resource budget exhausted")
            && stderr.contains("render")
            && stderr.contains("16"),
        "expected localized resource-limit diagnostic: {stderr}"
    );
    ensure!(
        !stderr.contains("s3cr3t"),
        "diagnostic leaked a template value"
    );
    ensure!(
        !workspace.path().join("build.ninja").exists(),
        "budget failure must precede generated output"
    );
    Ok(())
}
