//! End-to-end contracts for the `netsuke check` command.
//!
//! These run the built binary, because the properties under test are the ones
//! only the process boundary can show: the exit code, which stream each
//! document reaches, and that stdout stays empty when a run fails in JSON
//! mode.

use anyhow::{Context, Result, ensure};
use rstest::{fixture, rstest};
use serde_json::Value;
use tempfile::TempDir;
use test_support::fs as test_fs;
use test_support::netsuke::{NetsukeRun, run_netsuke_in};

/// A manifest whose only finding is a warning.
const WARNS: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: out.txt\n",
    "    command: \"cp $$SRC {{ outs }}\"\n",
);

/// A manifest that trips no rule.
const CLEAN: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: out.txt\n",
    "    command: \"touch {{ outs }}\"\n",
);

/// A workspace holding a manifest, kept alive for the duration of a test.
struct Workspace {
    /// The temporary directory backing the workspace.
    directory: TempDir,
}

impl Workspace {
    /// Create a workspace whose `Netsukefile` holds `manifest`.
    fn new(manifest: &str) -> Result<Self> {
        let directory = TempDir::new().context("create a workspace")?;
        test_fs::write(directory.path().join("Netsukefile"), manifest)
            .context("write the manifest")?;
        Ok(Self { directory })
    }

    /// Run `netsuke` with `args` in this workspace.
    fn run(&self, args: &[&str]) -> Result<NetsukeRun> {
        run_netsuke_in(self.directory.path(), args)
    }
}

/// A workspace whose manifest reports one warning.
#[fixture]
fn warning_workspace() -> Result<Workspace> {
    Workspace::new(WARNS)
}

/// A workspace whose manifest reports nothing.
#[fixture]
fn clean_workspace() -> Result<Workspace> {
    Workspace::new(CLEAN)
}

/// Parse a run's stdout as the JSON document it should have written.
fn document(run: &NetsukeRun) -> Result<Value> {
    serde_json::from_str(&run.stdout).context("parse the JSON document on stdout")
}

/// Parse a run's stderr as the JSON diagnostic document it should have written.
fn diagnostic(run: &NetsukeRun) -> Result<Value> {
    serde_json::from_str(&run.stderr).context("parse the JSON document on stderr")
}

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

#[rstest]
fn explain_publishes_the_rule_catalogue(clean_workspace: Result<Workspace>) -> Result<()> {
    let run = clean_workspace?.run(&["--json", "check", "--explain"])?;
    ensure!(run.success, "the run should succeed: {}", run.stderr);
    let document = document(&run)?;
    ensure!(
        document.pointer("/result/command") == Some(&Value::from("check-explain")),
        "the catalogue should name its own command"
    );
    let rules = document
        .pointer("/result/rules")
        .and_then(Value::as_array)
        .context("the catalogue should list rules")?;
    ensure!(!rules.is_empty(), "the catalogue should not be empty");
    for field in [
        "name",
        "category",
        "stage",
        "default_severity",
        "code",
        "summary",
        "rationale",
        "remediation",
        "url",
    ] {
        ensure!(
            rules.iter().all(|rule| rule.get(field).is_some()),
            "every catalogue entry should publish `{field}`"
        );
    }
    Ok(())
}

#[rstest]
fn explain_rejects_an_unknown_rule(clean_workspace: Result<Workspace>) -> Result<()> {
    let run = clean_workspace?.run(&["--json", "check", "--explain", "no-such-rule"])?;
    ensure!(!run.success, "the run should fail");
    ensure!(
        diagnostic(&run)?.pointer("/diagnostics/0/code")
            == Some(&Value::from("netsuke::lint::invalid_policy")),
        "the diagnostic should identify the failure: {}",
        run.stderr
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

/// Write `config` beside the workspace manifest and return its path argument.
fn write_config(workspace: &Workspace, config: &str) -> Result<String> {
    let path = workspace.directory.path().join("netsuke.toml");
    test_fs::write(&path, config).context("write the check configuration")?;
    path.to_str()
        .map(str::to_owned)
        .context("temporary config path should be UTF-8")
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
