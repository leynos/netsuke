//! Rule-explanation contracts for `netsuke check`.

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use serde_json::{Value, json};

use super::support::{Workspace, clean_workspace, diagnostic, document};

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

/// A named explanation renders only that rule's full metadata in both modes.
#[rstest]
fn explain_for_a_known_rule_is_precise_in_text_and_json(
    clean_workspace: Result<Workspace>,
) -> Result<()> {
    let workspace = clean_workspace?;
    let expected_text = [
        "manual-ninja-escape",
        "category: migration    stage: document    default: warning",
        "code: netsuke::lint::manual_ninja_escape",
        "summary: recipe doubles a dollar to escape it for Ninja",
        "rationale: Netsuke now escapes dollars at the Ninja writer boundary, after it has lowered its own placeholders. A recipe that still doubles a dollar reaches the shell as a literal `$$`, whose first two characters expand to the shell's process identifier rather than to the intended variable.",
        "remediation: Write the shell variable normally, for example `$PATH` rather than `$$PATH`.",
        "documentation: https://github.com/leynos/netsuke/blob/main/docs/netsuke-linter-rules.md#manual-ninja-escape",
    ];
    let text = workspace.run(&["check", "--explain", "manual-ninja-escape"])?;
    ensure!(
        text.success,
        "the text explanation should succeed: {}",
        text.stderr
    );
    for expected in expected_text {
        ensure!(
            text.stdout.contains(expected),
            "the text explanation should contain {expected:?}, got {}",
            text.stdout
        );
    }
    ensure!(
        !text.stdout.contains("legacy-placeholder"),
        "the text explanation must not include an unrelated rule: {}",
        text.stdout
    );

    let json_run = workspace.run(&["--json", "check", "--explain", "manual-ninja-escape"])?;
    ensure!(
        json_run.success,
        "the JSON explanation should succeed: {}",
        json_run.stderr
    );
    let json_document = document(&json_run)?;
    let rules = json_document
        .pointer("/result/rules")
        .and_then(Value::as_array)
        .context("the JSON explanation should carry rules")?;
    ensure!(rules.len() == 1, "expected exactly one rule, got {rules:?}");
    ensure!(
        rules.first()
            == Some(&json!({
                "name": "manual-ninja-escape",
                "category": "migration",
                "stage": "document",
                "default_severity": "warning",
                "code": "netsuke::lint::manual_ninja_escape",
                "summary": "recipe doubles a dollar to escape it for Ninja",
                "rationale": "Netsuke now escapes dollars at the Ninja writer boundary, after it has lowered its own placeholders. A recipe that still doubles a dollar reaches the shell as a literal `$$`, whose first two characters expand to the shell's process identifier rather than to the intended variable.",
                "remediation": "Write the shell variable normally, for example `$PATH` rather than `$$PATH`.",
                "url": "https://github.com/leynos/netsuke/blob/main/docs/netsuke-linter-rules.md#manual-ninja-escape",
            })),
        "the JSON explanation should publish the complete known-rule metadata: {rules:?}"
    );
    ensure!(
        !rules
            .iter()
            .any(|rule| rule.get("name") == Some(&Value::from("legacy-placeholder"))),
        "the JSON explanation must not include an unrelated rule: {rules:?}"
    );
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
