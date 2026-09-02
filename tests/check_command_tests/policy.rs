//! Policy-selection contracts for `netsuke check`.

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use serde_json::Value;

use super::support::{
    Workspace, clean_workspace, diagnostic, document, warning_workspace, write_config,
};

/// A finding at the threshold fails the run and moves to the diagnostic
/// branch, leaving stdout empty as the envelope contract requires.
#[rstest]
fn a_finding_at_the_threshold_fails_with_an_empty_stdout(
    warning_workspace: Result<Workspace>,
) -> Result<()> {
    let run = warning_workspace?.run(&["--json", "check", "--fail-on", "warning"])?;
    ensure!(!run.success, "the run should fail");
    ensure!(
        run.stdout.is_empty(),
        "a failing JSON run should leave stdout empty, got {}",
        run.stdout
    );
    let document = diagnostic(&run)?;
    let entry = document
        .pointer("/diagnostics/0")
        .context("the document should carry one top-level diagnostic")?;
    ensure!(
        entry.get("code") == Some(&Value::from("netsuke::lint::threshold_exceeded")),
        "the top-level entry should be the threshold summary"
    );
    let related = entry
        .get("related")
        .and_then(Value::as_array)
        .context("the summary should carry the findings")?;
    ensure!(related.len() == 1, "every finding should travel as related");
    ensure!(
        related
            .first()
            .and_then(|finding| finding.get("code"))
            .and_then(Value::as_str)
            .is_some_and(|code| code.starts_with("netsuke::lint::")),
        "a related entry should be a lint finding"
    );
    Ok(())
}

/// `--fail-on never` makes findings purely informational.
#[rstest]
fn never_failing_reports_without_failing(warning_workspace: Result<Workspace>) -> Result<()> {
    let run = warning_workspace?.run(&[
        "--json",
        "check",
        "--fail-on",
        "never",
        "--rule",
        "migration=error",
    ])?;
    ensure!(run.success, "the run should succeed: {}", run.stderr);
    ensure!(
        document(&run)?.pointer("/result/summary/error") == Some(&Value::from(1)),
        "the finding should still be reported at its configured severity"
    );
    Ok(())
}

#[rstest]
fn a_limit_bounds_the_findings_and_says_so(warning_workspace: Result<Workspace>) -> Result<()> {
    let run = warning_workspace?.run(&["--json", "check", "--limit", "0"])?;
    ensure!(run.success, "the run should succeed: {}", run.stderr);
    ensure!(
        document(&run)?.pointer("/result/truncated") == Some(&Value::from(false)),
        "an unbounded run truncates nothing"
    );

    let workspace = Workspace::new(concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: out.txt\n",
        "    command: \"cp $$A $$B $$C {{ outs }}\"\n",
    ))?;
    let bounded = workspace.run(&["--json", "check", "--limit", "1"])?;
    ensure!(
        bounded.success,
        "the run should succeed: {}",
        bounded.stderr
    );
    let document = document(&bounded)?;
    ensure!(
        document.pointer("/result/truncated") == Some(&Value::from(true)),
        "a bounded run should say it truncated"
    );
    ensure!(
        document.pointer("/result/summary/omitted") == Some(&Value::from(2)),
        "the summary should count what was omitted: {}",
        bounded.stdout
    );
    Ok(())
}

/// An unknown selector must fail loudly rather than silently changing the run.
#[rstest]
#[case("--rule", "no-such-rule=error", "netsuke::lint::invalid_policy")]
#[case("--rule", "background-job", "netsuke::lint::invalid_policy")]
#[case("--fail-on", "fatal", "netsuke::lint::invalid_policy")]
fn an_invalid_policy_fails_with_a_diagnostic(
    clean_workspace: Result<Workspace>,
    #[case] flag: &str,
    #[case] value: &str,
    #[case] code: &str,
) -> Result<()> {
    let run = clean_workspace?.run(&["--json", "check", flag, value])?;
    ensure!(!run.success, "the run should fail");
    ensure!(run.stdout.is_empty(), "stdout should stay empty");
    ensure!(
        diagnostic(&run)?.pointer("/diagnostics/0/code") == Some(&Value::from(code)),
        "the diagnostic should identify the failure: {}",
        run.stderr
    );
    Ok(())
}

/// A human check fails at its selected threshold without writing a result.
#[rstest]
fn human_failure_at_the_warning_threshold_uses_stderr(
    warning_workspace: Result<Workspace>,
) -> Result<()> {
    let run = warning_workspace?.run(&[
        "--locale",
        "en-US",
        "--color",
        "never",
        "check",
        "--fail-on",
        "warning",
    ])?;
    ensure!(!run.success, "the warning threshold should fail");
    ensure!(
        run.stdout.is_empty(),
        "a threshold failure should not write a successful result: {}",
        run.stdout
    );
    for expected in ["manual-ninja-escape", "Lint findings reached"] {
        ensure!(
            run.stderr.contains(expected),
            "the failure should contain {expected:?}, got {}",
            run.stderr
        );
    }
    Ok(())
}

/// A `[cmds.check]` table supplies the policy when the caller gives none.
#[rstest]
fn configuration_supplies_the_check_policy(warning_workspace: Result<Workspace>) -> Result<()> {
    let workspace = warning_workspace?;
    let config = write_config(&workspace, "[cmds.check]\nfail_on = \"warning\"\n")?;
    let run = workspace.run(&["--config", &config, "--json", "check"])?;
    ensure!(
        !run.success,
        "the configured threshold should fail the run: {}",
        run.stdout
    );
    Ok(())
}

/// An explicit flag outranks the configuration file.
///
/// The check is written against a value that equals the built-in default,
/// because that is where a merge keyed on "differs from the default" rather
/// than on "supplied on the command line" would silently keep the
/// configuration's value.
#[rstest]
fn an_explicit_flag_outranks_the_configuration(warning_workspace: Result<Workspace>) -> Result<()> {
    let workspace = warning_workspace?;
    let config = write_config(&workspace, "[cmds.check]\nfail_on = \"warning\"\n")?;
    let run = workspace.run(&["--config", &config, "--json", "check", "--fail-on", "error"])?;
    ensure!(
        run.success,
        "the explicit threshold should win over the configured one: {}",
        run.stderr
    );
    Ok(())
}

/// Configured rule selectors and limits reach the run.
#[rstest]
fn configuration_supplies_selectors_and_limits(warning_workspace: Result<Workspace>) -> Result<()> {
    let workspace = warning_workspace?;
    let config = write_config(
        &workspace,
        "[cmds.check]\nrule = [\"migration=off\"]\nlimit = 1\n",
    )?;
    let run = workspace.run(&["--config", &config, "--json", "check"])?;
    ensure!(run.success, "configured selectors: {}", run.stderr);
    let findings = document(&run)?
        .pointer("/result/findings")
        .and_then(Value::as_array)
        .cloned()
        .context("the result should carry a findings array")?;
    ensure!(
        findings.is_empty(),
        "the configured selector should disable the rule, got {findings:?}"
    );
    Ok(())
}

/// An explicit selector restores a category configuration disabled.
#[rstest]
fn an_explicit_rule_outranks_the_configuration(warning_workspace: Result<Workspace>) -> Result<()> {
    let workspace = warning_workspace?;
    let config = write_config(&workspace, "[cmds.check]\nrule = [\"migration=off\"]\n")?;
    let run = workspace.run(&[
        "--config",
        &config,
        "--json",
        "check",
        "--rule",
        "migration=warning",
    ])?;
    ensure!(
        run.success,
        "explicit selector should succeed: {}",
        run.stderr
    );
    let document = document(&run)?;
    let findings = document
        .pointer("/result/findings")
        .and_then(Value::as_array)
        .context("the result should carry a findings array")?;
    ensure!(
        findings.iter().any(|finding| {
            finding.get("code") == Some(&Value::from("netsuke::lint::manual_ninja_escape"))
        }),
        "the explicit selector should restore the migration finding: {findings:?}"
    );
    Ok(())
}

/// A configured limit bounds the published array, but not the whole-run summary.
#[test]
fn configuration_limit_bounds_the_check_document() -> Result<()> {
    let workspace = Workspace::new(concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: out.txt\n",
        "    command: \"cp $$A $$B $$C {{ outs }}\"\n",
    ))?;
    let config = write_config(&workspace, "[cmds.check]\nlimit = 1\n")?;
    let run = workspace.run(&["--config", &config, "--json", "check"])?;
    ensure!(
        run.success,
        "configured limit should succeed: {}",
        run.stderr
    );
    let document = document(&run)?;
    ensure!(
        document.pointer("/result/truncated") == Some(&Value::from(true)),
        "configured limit should mark the result as truncated: {}",
        run.stdout
    );
    ensure!(
        document.pointer("/result/summary/reported") == Some(&Value::from(1)),
        "configured limit should keep one finding: {}",
        run.stdout
    );
    ensure!(
        document.pointer("/result/summary/omitted") == Some(&Value::from(2)),
        "configured limit should account for omitted findings: {}",
        run.stdout
    );
    Ok(())
}

/// An explicit limit overrides the configured limit in the result summary.
#[test]
fn an_explicit_limit_outranks_the_configuration() -> Result<()> {
    let workspace = Workspace::new(concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: out.txt\n",
        "    command: \"cp $$A $$B $$C {{ outs }}\"\n",
    ))?;
    let config = write_config(&workspace, "[cmds.check]\nlimit = 1\n")?;
    let run = workspace.run(&["--config", &config, "--json", "check", "--limit", "2"])?;
    ensure!(run.success, "explicit limit should succeed: {}", run.stderr);
    let document = document(&run)?;
    let summary = document
        .pointer("/result/summary")
        .context("the result should carry a summary")?;
    let reported = summary
        .get("reported")
        .and_then(Value::as_u64)
        .context("the summary should report its retained findings")?;
    let omitted = summary
        .get("omitted")
        .and_then(Value::as_u64)
        .context("the summary should report its omitted findings")?;
    let total = ["error", "warning", "advice"]
        .into_iter()
        .try_fold(0_u64, |count, severity| {
            summary
                .get(severity)
                .and_then(Value::as_u64)
                .map(|findings| count + findings)
                .with_context(|| format!("the summary should count {severity} findings"))
        })?;
    let expected_omitted = total
        .checked_sub(reported)
        .context("reported findings should not exceed the whole-run total")?;
    ensure!(
        reported == 2,
        "the explicit limit should retain two findings"
    );
    ensure!(
        omitted == expected_omitted,
        "the omitted count should describe the whole-run total"
    );
    ensure!(
        document.pointer("/result/truncated") == Some(&Value::from(true)),
        "the explicit limit should mark the result as truncated: {}",
        run.stdout
    );
    Ok(())
}
