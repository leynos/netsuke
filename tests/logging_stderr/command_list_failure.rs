//! Runtime diagnostics for failed entries in command-list recipes.

use super::support::open_workspace;
use anyhow::{Context, Result, ensure};
use cap_std::fs_utf8::Dir;
use netsuke::runner::NINJA_ENV;
use serde_json::Value;
use tempfile::TempDir;
use test_support::ninja::ninja_integration_workspace;

const FAILURE_PREFIX: &str = "netsuke command-list failure: action ";

fn identifies_entry(message: &str, entry: usize) -> bool {
    message.contains(FAILURE_PREFIX) && message.contains(&format!(", entry {entry}"))
}

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
      - "echo unexpected >> $out"
"#,
        )
        .as_bytes(),
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
    let Some(temp) = failing_command_list_workspace("echo first > $out")? else {
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
    let Some(temp) = failing_command_list_workspace("echo first > $out")? else {
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
        !stderr.contains("false"),
        "JSON attribution must not expose the command text: {stderr}"
    );
    Ok(())
}

#[test]
fn failed_command_list_entry_is_attributed_in_tracing_output() -> Result<()> {
    let Some(temp) = failing_command_list_workspace("echo first > $out")? else {
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
    Ok(())
}

#[test]
fn large_command_stdout_retains_command_list_failure_attribution() -> Result<()> {
    let Some(temp) = failing_command_list_workspace("echo first > $out && yes x | head -c 262144")?
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
