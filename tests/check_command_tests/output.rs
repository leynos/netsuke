//! Process-boundary output contracts for `netsuke check`.

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use serde_json::Value;
use tempfile::TempDir;
use test_support::fs as test_fs;
use test_support::netsuke::run_netsuke_in;

use super::support::{Workspace, clean_workspace, diagnostic, document, warning_workspace};

/// A finding below the failure threshold is reported without failing the run.
#[rstest]
fn a_finding_below_the_threshold_succeeds(warning_workspace: Result<Workspace>) -> Result<()> {
    let run = warning_workspace?.run(&["--json", "check"])?;
    ensure!(run.success, "the run should succeed: {}", run.stderr);
    let document = document(&run)?;
    ensure!(
        document.pointer("/result/command") == Some(&Value::from("check")),
        "the result should name the command"
    );
    ensure!(
        document.pointer("/result/status") == Some(&Value::from("pass")),
        "the result should report the verdict"
    );
    ensure!(
        document.pointer("/result/summary/warning") == Some(&Value::from(1)),
        "the summary should count the warning: {}",
        run.stdout
    );
    ensure!(
        run.stderr.is_empty() || !run.stderr.contains("diagnostics"),
        "a passing run should write no diagnostic document"
    );
    Ok(())
}

/// Both branches must carry the same per-finding shape, so a consumer parses
/// one representation and selects the array by presence.
#[rstest]
fn both_branches_carry_the_same_finding_shape(warning_workspace: Result<Workspace>) -> Result<()> {
    let workspace = warning_workspace?;
    let passing = workspace.run(&["--json", "check"])?;
    let failing = workspace.run(&["--json", "check", "--fail-on", "warning"])?;
    let from_result = document(&passing)?
        .pointer("/result/findings/0")
        .cloned()
        .context("the passing run should report a finding")?;
    let from_diagnostic = diagnostic(&failing)?
        .pointer("/diagnostics/0/related/0")
        .cloned()
        .context("the failing run should report a finding")?;
    ensure!(
        from_result == from_diagnostic,
        "the two branches disagree:\n{from_result:#}\n{from_diagnostic:#}"
    );
    Ok(())
}

#[rstest]
fn a_clean_manifest_reports_nothing(clean_workspace: Result<Workspace>) -> Result<()> {
    let run = clean_workspace?.run(&["--json", "check"])?;
    ensure!(run.success, "the run should succeed: {}", run.stderr);
    let findings = document(&run)?
        .pointer("/result/findings")
        .and_then(Value::as_array)
        .cloned()
        .context("the result should carry a findings array")?;
    ensure!(findings.is_empty(), "got {findings:?}");
    Ok(())
}

/// Check manifests without running their recipes or creating build output.
#[test]
fn check_is_read_only_at_the_process_boundary() -> Result<()> {
    let workspace = Workspace::new(concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: generated.txt\n",
        "    command: \"touch recipe-ran-marker && touch {{ outs }}\"\n",
    ))?;
    let run = workspace.run(&["--json", "check"])?;
    ensure!(run.success, "check should succeed: {}", run.stderr);
    for path in ["recipe-ran-marker", "generated.txt", "build.ninja"] {
        ensure!(
            !test_fs::exists(workspace.directory.path().join(path)),
            "check should not create {path}"
        );
    }
    Ok(())
}

/// Check refuses a template helper with side effects instead of running it.
///
/// `check` loads through the source-preserving query loader, so it must apply
/// the same restricted standard library as the other query commands: a
/// `shell` helper in an evaluated field is rejected by name, never runs, and
/// the failure is a load error rather than a lint finding.
#[test]
fn check_refuses_an_effectful_template_helper() -> Result<()> {
    let workspace = Workspace::new(concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: hello\n",
        "    description: >-\n",
        "      {{ 'ignored' | shell('touch helper-ran-marker') }}\n",
        "    command: echo hi\n",
    ))?;
    let run = workspace.run(&["--json", "check"])?;
    ensure!(
        !run.success,
        "check should refuse the effectful helper: {}",
        run.stdout
    );
    for path in ["helper-ran-marker", ".netsuke"] {
        ensure!(
            !test_fs::exists(workspace.directory.path().join(path)),
            "check must not run the helper or create {path}"
        );
    }
    ensure!(
        run.stderr.contains("shell"),
        "the diagnostic should name the refused helper, got {}",
        run.stderr
    );
    // A template load failure carries no diagnostic code, so the assertion is
    // only that no lint code appears.
    let document = diagnostic(&run)?;
    let code = document
        .pointer("/diagnostics/0/code")
        .and_then(Value::as_str);
    ensure!(
        !code.is_some_and(|code| code.starts_with("netsuke::lint::")),
        "a refused helper is a load failure, not a lint finding, got {document}"
    );
    Ok(())
}

/// Human output goes to stdout and carries the source snippet a reader needs.
#[rstest]
fn human_output_shows_the_offending_source(warning_workspace: Result<Workspace>) -> Result<()> {
    let run = warning_workspace?.run(&["--locale", "en-US", "--color", "never", "check"])?;
    ensure!(run.success, "the run should succeed: {}", run.stderr);
    for expected in ["manual-ninja-escape", "$$SRC", "Lint results"] {
        ensure!(
            run.stdout.contains(expected),
            "human output should contain {expected:?}, got {}",
            run.stdout
        );
    }
    Ok(())
}

/// Help for `check` renders without requiring a manifest or build tools.
#[test]
fn help_check_renders_the_check_command_reference() -> Result<()> {
    let directory = TempDir::new().context("create a workspace")?;
    let run = run_netsuke_in(directory.path(), &["help", "check"])?;
    ensure!(run.success, "check help should succeed: {}", run.stderr);
    for expected in [
        "Analyse the selected manifest for constructs that parse but are likely erroneous, unsafe,",
        "non-portable, or hostile to caching.",
        "Usage: check [OPTIONS]",
        "--rule <NAME=SEVERITY>",
        "--fail-on <SEVERITY>",
        "--limit <N>",
        "--explain [<RULE>]",
    ] {
        ensure!(
            run.stdout.contains(expected),
            "check help should contain {expected:?}, got {}",
            run.stdout
        );
    }
    Ok(())
}

/// A missing manifest is an ordinary command failure, not a lint finding.
#[test]
fn a_missing_manifest_fails_before_any_rule_runs() -> Result<()> {
    let directory = TempDir::new().context("create a workspace")?;
    let run = run_netsuke_in(directory.path(), &["--json", "check"])?;
    ensure!(!run.success, "the run should fail");
    let code = diagnostic(&run)?
        .pointer("/diagnostics/0/code")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .context("the diagnostic should carry a code")?;
    ensure!(
        !code.starts_with("netsuke::lint::"),
        "a missing manifest is not a lint failure, got {code}"
    );
    Ok(())
}
