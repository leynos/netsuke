//! The `shell_join` intermediate-byte ceiling on the manifest-query surface.
//!
//! `help targets` renders descriptions through the restricted query stdlib,
//! which registers `shell_join` from the same implementation the build surface
//! uses. Issue #970 reported that a compact manifest repeating a reference to
//! one generated string made the filter allocate an encoded copy of every
//! member and then the whole joined string, before any rendered-output budget
//! applied.
//!
//! These cases pin the helper-local ceiling that now fails first, that the
//! failure happens while rendering rather than during an execution, and that an
//! ordinary join of the same shape still renders.
//!
//! [`run_query`]: super::run_query

use anyhow::{Context, Result, ensure};
use serde_json::Value;
use test_support::fs as test_fs;

use super::run_query_retaining_workspace;

/// The stable diagnostic code the ceiling error carries.
const LIMIT_CODE: &str = "[netsuke::jinja::shell::join_limit]";

/// The ceiling in bytes, restated here so the probe does not read the
/// implementation it is meant to check.
const LIMIT_TEXT: &str = "1048576";

/// The trigger issue #970 reported: one generated word joined three times.
///
/// The manifest is compact; only the rendered sequence is large. The `length`
/// filter is what the report used to make the output small enough to display,
/// and it must not be reached because the join itself fails first.
const AMPLIFIED: &str = concat!(
    "{% set word = 'a' * 400000 %}",
    "{{ [word, word, word] | shell_join | length }}"
);

/// The same shape inside the ceiling: three references to a 1000-byte word.
///
/// The join is 1000 + 1 + 1000 + 1 + 1000 bytes, so the control proves the
/// ceiling is a bound and not a blanket refusal of repetition.
const ACCEPTED: &str = concat!(
    "{% set word = 'a' * 1000 %}",
    "{{ [word, word, word] | shell_join | length }}"
);

/// Parse one failed query's diagnostic cause chain from its stderr.
fn causes(stderr: &str) -> Result<Vec<String>> {
    let document: Value = serde_json::from_str(stderr)
        .with_context(|| format!("stderr should be one JSON document: {stderr}"))?;
    let causes = document
        .pointer("/diagnostics/0/causes")
        .and_then(Value::as_array)
        .context("diagnostic should carry its cause chain")?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    Ok(causes)
}

/// A repeated reference cannot amplify past the helper-local ceiling.
#[test]
fn a_repeated_reference_cannot_amplify_past_the_ceiling() -> Result<()> {
    let (workspace, run) = run_query_retaining_workspace(AMPLIFIED)?;
    ensure!(
        !run.success,
        "the amplified join must fail the query, not render it: {}",
        run.stdout
    );
    ensure!(
        !run.stdout.contains("description"),
        "a failed query must not emit a catalogue: {}",
        run.stdout
    );

    let causes = causes(&run.stderr)?;
    let reported = causes.join("\n");
    ensure!(
        reported.contains(LIMIT_CODE),
        "the failure should name the shell_join ceiling: {causes:?}"
    );
    ensure!(
        reported.contains(LIMIT_TEXT),
        "the failure should state the ceiling it enforced: {causes:?}"
    );
    ensure!(
        !reported.contains("manifest_budget_exceeded"),
        "the helper-local ceiling must fire before any rendered-output budget: {causes:?}"
    );
    ensure!(
        !reported.contains("aaaa"),
        "the diagnostic must not echo the content it refused: {causes:?}"
    );
    ensure!(
        !test_fs::exists(workspace.path().join(".netsuke")),
        "a failed query must not create build state"
    );
    Ok(())
}

/// A join within the ceiling still renders through the query surface.
#[test]
fn a_join_within_the_ceiling_still_renders() -> Result<()> {
    let run = super::run_query(ACCEPTED)?;
    ensure!(
        run.success,
        "the control join should render: {}",
        run.stderr
    );
    let document: Value = serde_json::from_str(&run.stdout)
        .with_context(|| format!("stdout should be one JSON document: {}", run.stdout))?;
    let rendered = document
        .pointer("/result/targets/0/description")
        .and_then(Value::as_str)
        .context("the catalogue should carry the target description")?;
    ensure!(
        rendered == "3002",
        "three 1000-byte words and two separators render 3002 bytes, got {rendered:?}"
    );
    Ok(())
}
