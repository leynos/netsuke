//! Runtime diagnostics for failed entries in command-list recipes.

use super::support::{normalized_diagnostic_fragment, open_workspace};
use crate::command_list_failure_support::{
    FAILURE_PREFIX, contains_full_fingerprint, contains_target_clause, identifies_entry,
};
use anyhow::{Context, Result, ensure};
use cap_std::fs_utf8::Dir;
use netsuke::runner::NINJA_ENV;
use serde_json::Value;
use tempfile::TempDir;
use test_support::ninja::ninja_integration_workspace;

fn failing_command_list_workspace(first_entry: &str) -> Result<Option<TempDir>> {
    let temp = match ninja_integration_workspace() {
        Ok(temp) => temp,
        Err(error) => {
            tracing::warn!(%error, "skipping command-list failure attribution test: Ninja unavailable");
            return Ok(None);
        }
    };
    let workspace: Dir = open_workspace(&temp)?;
    workspace.write(
        "Netsukefile",
        format!(
            r#"
netsuke_version: "1.0.0"
targets:
  - name: result.txt
    command:
      - "{first_entry}"
      - "false"
      - "echo unexpected >> {{{{ outs }}}}"
"#,
        )
        .as_bytes(),
    )?;
    Ok(Some(temp))
}

/// Build a manifest with two targets sharing one failing command-list action.
fn shared_command_list_workspace() -> Result<Option<TempDir>> {
    let temp = match ninja_integration_workspace() {
        Ok(temp) => temp,
        Err(error) => {
            tracing::warn!(%error, "skipping shared command-list attribution test: Ninja unavailable");
            return Ok(None);
        }
    };
    let workspace: Dir = open_workspace(&temp)?;
    workspace.write(
        "Netsukefile",
        concat!(
            "netsuke_version: '1.0.0'\n",
            "targets:\n",
            "  - name: left\n",
            "    command: ['echo first', 'false', 'touch unexpected']\n",
            "  - name: right\n",
            "    command: ['echo first', 'false', 'touch unexpected']\n",
            "  - name: all\n",
            "    deps: [left, right]\n",
            "    command: 'true'\n",
            "defaults: [all]\n",
        ),
    )?;
    Ok(Some(temp))
}

fn run_failing_build(temp: &TempDir, arguments: &[&str]) -> Result<std::process::Output> {
    assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .env(NINJA_ENV, "ninja")
        .args(arguments)
        .output()
        .context("run failing command-list build")
}

#[test]
fn failed_command_list_entry_is_attributed_in_human_output() -> Result<()> {
    let Some(temp) = failing_command_list_workspace("echo first > {{ outs }}")? else {
        return Ok(());
    };
    let output = run_failing_build(&temp, &["--progress", "never", "build"])?;
    ensure!(
        !output.status.success(),
        "a failing list entry must fail the build"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be valid UTF-8")?;
    ensure!(
        identifies_entry(&stderr, 2),
        "human diagnostics should name the bounded failing entry: {stderr}"
    );
    ensure!(
        contains_full_fingerprint(&stderr),
        "human diagnostics should retain the full action fingerprint: {stderr}"
    );
    ensure!(
        contains_target_clause(&stderr, "target \"result.txt\""),
        "human diagnostics should name the resolved target: {stderr}"
    );
    insta::assert_snapshot!(
        "command_list_failure_human_diagnostic",
        normalized_diagnostic_fragment(&stderr, FAILURE_PREFIX)
            .context("human diagnostic should contain its marker line")?
    );
    let output_file = open_workspace(&temp)?
        .read_to_string("result.txt")
        .context("read the partial command-list output")?;
    ensure!(
        output_file == "first\n",
        "a failure must prevent subsequent command-list entries from running, got {output_file:?}"
    );
    Ok(())
}

#[test]
fn failed_command_list_entry_is_attributed_in_json_diagnostics() -> Result<()> {
    let Some(temp) = failing_command_list_workspace("echo first > {{ outs }}")? else {
        return Ok(());
    };
    let output = run_failing_build(&temp, &["--json", "build"])?;
    ensure!(
        !output.status.success(),
        "a failing list entry must fail the build"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be valid UTF-8")?;
    let diagnostics: Value = serde_json::from_str(&stderr).context("stderr should be JSON")?;
    ensure!(
        identifies_entry(&diagnostics.to_string(), 2),
        "JSON diagnostics should retain bounded entry attribution: {diagnostics}"
    );
    ensure!(
        contains_full_fingerprint(&stderr),
        "JSON diagnostics should retain the full action fingerprint: {stderr}"
    );
    ensure!(
        contains_target_clause(&diagnostics.to_string(), "target \"result.txt\""),
        "JSON diagnostics should name the resolved target: {diagnostics}"
    );
    insta::assert_snapshot!(
        "command_list_failure_json_diagnostic",
        normalized_diagnostic_fragment(&stderr, FAILURE_PREFIX)
            .context("JSON diagnostic should contain its marker line")?
    );
    ensure!(
        !stderr.contains("false"),
        "JSON attribution must not expose the command text: {stderr}"
    );
    Ok(())
}

#[test]
fn failed_command_list_entry_is_attributed_in_tracing_output() -> Result<()> {
    let Some(temp) = failing_command_list_workspace("echo first > {{ outs }}")? else {
        return Ok(());
    };
    let output = run_failing_build(&temp, &["--verbose", "--progress", "never", "build"])?;
    ensure!(
        !output.status.success(),
        "a failing list entry must fail the build"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be valid UTF-8")?;
    ensure!(
        stderr.contains("command_list_failure") && identifies_entry(&stderr, 2),
        "tracing should record the bounded command-list failure context: {stderr}"
    );
    ensure!(
        contains_full_fingerprint(&stderr),
        "tracing should retain the full action fingerprint: {stderr}"
    );
    ensure!(
        stderr.contains("command_list_target")
            && contains_target_clause(&stderr, "target \"result.txt\""),
        "tracing should name the resolved target: {stderr}"
    );
    insta::assert_snapshot!(
        "command_list_failure_tracing_diagnostic",
        normalized_diagnostic_fragment(&stderr, "command_list_action=")
            .context("tracing should contain the target field")?
    );
    Ok(())
}

#[test]
fn shared_command_list_failure_reports_ambiguous_target_candidates() -> Result<()> {
    let Some(temp) = shared_command_list_workspace()? else {
        return Ok(());
    };
    let output = run_failing_build(&temp, &["--progress", "never", "build"])?;
    ensure!(
        !output.status.success(),
        "a failing shared command list must fail the build"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be valid UTF-8")?;
    ensure!(
        identifies_entry(&stderr, 2),
        "ambiguous attribution should retain the failing entry: {stderr}"
    );
    ensure!(
        contains_full_fingerprint(&stderr),
        "ambiguous attribution should retain the full fingerprint: {stderr}"
    );
    ensure!(
        contains_target_clause(&stderr, "targets \"left\", \"right\""),
        "shared actions should report sorted candidate targets: {stderr}"
    );
    ensure!(
        !contains_target_clause(&stderr, "target \"all\""),
        "the serial aggregate must not replace the failing direct targets: {stderr}"
    );
    Ok(())
}

#[test]
fn large_command_stdout_retains_command_list_failure_attribution() -> Result<()> {
    let Some(temp) = failing_command_list_workspace(
        "echo first > {{ outs }} && i=0; while [ $i -lt 65536 ]; do printf 'xxxx\\\\n'; i=$((i + 1)); done",
    )?
    else {
        return Ok(());
    };
    let output = run_failing_build(&temp, &["--progress", "never", "build"])?;
    ensure!(
        !output.status.success(),
        "a failing list entry must fail the build"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be valid UTF-8")?;
    ensure!(
        identifies_entry(&stderr, 2),
        "large command stdout must not hide the bounded failing entry: {stderr}"
    );
    Ok(())
}
