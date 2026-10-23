//! Check executable documentation contracts for the manifest linter.

use super::*;

/// The documented `netsuke check` invocation must run and report cleanly.
///
/// The documented command is run as written, then its result is read from
/// the JSON document rather than from localized prose, so a wording change
/// cannot break the test and a broken report cannot pass it.
#[test]
fn check_example_reports_a_clean_manifest() -> Result<()> {
    let example = documented_example("guide-check-command")?;
    ensure!(example.body == "netsuke check\n", "check example drifted");
    let workspace = manifest_workspace("guide-first-build-manifest")?;
    let run = run_netsuke_in(workspace.path(), &["check"])?;
    assert_success(&run, "check example")?;
    let json = run_netsuke_in(workspace.path(), &["--json", "check"])?;
    assert_success(&json, "check example in JSON mode")?;
    let document: Value =
        serde_json::from_str(&json.stdout).context("parse the check result document")?;
    ensure!(
        document.pointer("/result/status") == Some(&Value::from("pass")),
        "the documented manifest should pass, got {document}"
    );
    ensure!(
        document.pointer("/result/summary/error") == Some(&Value::from(0)),
        "a passing report should carry no error findings, got {document}"
    );
    ensure!(
        document
            .pointer("/result/findings")
            .is_some_and(Value::is_array),
        "the result should carry a findings array, got {document}"
    );
    Ok(())
}

/// Every documented `netsuke check` invocation must run as written.
///
/// The `--explain` catalogue is checked for a rule it must contain, and the
/// policy example for the effect its selectors claim: a `clarity=off` category
/// selector followed by a rule selector that promotes one of that category's
/// rules to `error` must still report that rule.
#[test]
fn check_explain_and_policy_examples_run() -> Result<()> {
    let workspace = manifest_workspace("guide-first-build-manifest")?;
    let explain = documented_example("guide-check-explain")?;
    for line in explain.body.lines() {
        let arguments: Vec<&str> = line.split_whitespace().skip(1).collect();
        let run = run_netsuke_in(workspace.path(), &arguments)?;
        assert_success(&run, line)?;
        ensure!(
            run.stdout.contains("directory-dep-not-order-only"),
            "`{line}` should describe the rule it names, got {}",
            run.stdout
        );
    }

    // The documented selectors disable the `clarity` category, then promote one
    // of its rules back to `error` under a `warning` threshold. Running them
    // against a manifest that violates that rule is what shows the selectors
    // take effect: asserting only that the command succeeds would pass just as
    // well if they were ignored entirely.
    let policy = documented_example("guide-check-policy")?;
    let arguments: Vec<&str> = policy.body.split_whitespace().skip(1).collect();
    let policy_workspace = tempfile::tempdir().context("create the policy workspace")?;
    test_fs::write(
        policy_workspace.path().join("Netsukefile"),
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: output.txt\n",
            "    sources: input.txt\n",
            "    command: \"cp input.txt output.txt\"\n",
        ),
    )
    .context("write the policy fixture manifest")?;
    let run = run_netsuke_in(policy_workspace.path(), &arguments)?;
    ensure!(
        !run.success,
        "the promoted rule should reach the threshold: {}{}",
        run.stdout,
        run.stderr
    );
    ensure!(
        format!("{}{}", run.stdout, run.stderr).contains("literal-recipe-path"),
        "the promoted rule should be the one reported"
    );
    Ok(())
}

/// The documented configuration example must be accepted and take effect.
///
/// The file turns the opt-in `unreachable-target` rule on at `warning` and
/// fails on warnings. A manifest with an unreachable target therefore passes
/// without the file and fails with it, naming that rule: asserting only that
/// the file is accepted would pass just as well if it were ignored.
#[test]
fn check_configuration_example_is_accepted() -> Result<()> {
    documented_configuration_example_is_accepted(
        "guide-check-config",
        "check.toml",
        &["--json", "check", "--explain"],
        "check configuration example",
    )?;
    let example = documented_example("guide-check-config")?;
    let workspace = tempfile::tempdir().context("create the configuration workspace")?;
    test_fs::write(
        workspace.path().join("Netsukefile"),
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: app\n",
            "    command: \"build > {{ outs }}\"\n",
            "  - name: scratch.txt\n",
            "    command: \"echo scratch > {{ outs }}\"\n",
            "defaults:\n",
            "  - app\n",
        ),
    )
    .context("write the configuration fixture manifest")?;
    let config_path = workspace.path().join("check.toml");
    test_fs::write(&config_path, example.body).context("write the documented configuration")?;
    let config = config_path
        .to_str()
        .context("temporary config path should be UTF-8")?;

    let unconfigured = run_netsuke_in(workspace.path(), &["--json", "check"])?;
    assert_success(&unconfigured, "check without the configuration")?;
    let configured = run_netsuke_in(workspace.path(), &["--config", config, "--json", "check"])?;
    ensure!(
        !configured.success,
        "the configured threshold should fail the unreachable target: {}{}",
        configured.stdout,
        configured.stderr
    );
    ensure!(
        format!("{}{}", configured.stdout, configured.stderr).contains("unreachable-target"),
        "the configured rule should be the one reported"
    );
    Ok(())
}

/// The documented suppression comment must silence the finding it names.
///
/// Without the directive the manifest reports `background-job`; with it, the
/// run is clean. Asserting both directions is what proves the example teaches
/// a working suppression rather than a manifest that never had a finding.
#[test]
fn check_suppression_example_silences_its_finding() -> Result<()> {
    let workspace = manifest_workspace("guide-check-suppression")?;
    let clean = run_netsuke_in(workspace.path(), &["--json", "check", "--fail-on", "never"])?;
    assert_success(&clean, "suppressed check example")?;
    let document: Value =
        serde_json::from_str(&clean.stdout).context("parse the check result document")?;
    let findings = document
        .pointer("/result/findings")
        .and_then(Value::as_array)
        .context("the result should carry a findings array")?;
    ensure!(
        findings.is_empty(),
        "the documented directive should silence every finding, got {findings:?}"
    );
    ensure!(
        document.pointer("/result/summary/suppressed") == Some(&Value::from(1)),
        "the directive should be recorded as having suppressed one finding"
    );
    Ok(())
}
