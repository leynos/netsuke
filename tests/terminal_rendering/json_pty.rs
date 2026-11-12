//! Assert JSON mode's observable contract under a pseudo-terminal attachment.
//!
//! The policy under test is `--json --color always`: a deliberately hostile
//! combination, because `always` is the one colour policy that would style every
//! other surface, and JSON mode must override it. Each case attaches one stream
//! to a pseudo-terminal and leaves the other piped, so both halves of the split
//! are exercised rather than one standing in for the other.
//!
//! Three things are asserted about a successful run, and they are separate
//! claims. The stream that carries the result holds exactly one versioned
//! document, matched on its `schema_version` and `result.command` fields rather
//! than on the mere presence of braces, which the document's pretty-printing
//! would satisfy by accident. The stream that carries diagnostics is empty once
//! trailing whitespace is trimmed, so no stage label and no tracing marker
//! appears there. And no escape byte reaches either captured stream, which is
//! what `--color always` would otherwise break.
//!
//! Every case runs `--verbose` so the verbosity an operator would reach for is
//! the one under test: a JSON run that only suppressed tracing at the default
//! level would still leak a `TRACE` line here. The contrast is measured on the
//! same fixture rather than argued from the source — [`human_mode_contrast`]
//! runs the equivalent human-mode command and shows it emitting both the stage
//! label and tracing output, so the JSON assertions are known not to be
//! vacuous.

use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use rstest::rstest;
use test_support::terminal::{Attach, classify};

use super::support::{TerminalWorkspace, visible_text};

/// The stage label a non-JSON run prints, which JSON mode must suppress.
const STAGE_LABEL: &str = "Stage 1/6";

/// Return the JSON-carrying and diagnostic-carrying streams for an attachment.
///
/// `run_pty` captures the attached stream in `terminal` and the other in
/// `piped`, so the JSON stream's identity depends on which one is attached.
fn stream_roles(attach: Attach) -> (&'static str, &'static str) {
    match attach {
        Attach::StdoutOnly => ("terminal", "piped"),
        Attach::Both | Attach::StderrOnly => ("piped", "terminal"),
    }
}

/// Return the captured bytes for one of the two named streams.
fn stream_bytes<'a>(output: &'a test_support::terminal::PtyOutput, name: &str) -> &'a [u8] {
    if name == "terminal" {
        &output.terminal
    } else {
        &output.piped
    }
}

/// Assert that `bytes` is one JSON result document naming `command`.
fn assert_result_document(bytes: &[u8], command_name: &str) -> Result<()> {
    let document: serde_json::Value = serde_json::from_slice(bytes).with_context(|| {
        format!(
            "the result stream should carry one JSON document, got {:?}",
            String::from_utf8_lossy(bytes)
        )
    })?;
    // Navigate with `get` rather than subscripting: a missing key must read as
    // "absent", not panic, so a malformed document fails on the assertion
    // below instead of aborting the test.
    let schema_version = document
        .get("schema_version")
        .and_then(serde_json::Value::as_u64);
    let command = document
        .get("result")
        .and_then(|result| result.get("command"))
        .and_then(serde_json::Value::as_str);
    expect_that!(schema_version, some(eq(1)));
    expect_that!(command, some(eq(command_name)));
    Ok(())
}

#[googletest::gtest]
#[rstest]
#[case::stdout_terminal(Attach::StdoutOnly)]
#[case::stderr_terminal(Attach::StderrOnly)]
fn json_mode_satisfies_its_contract_on_either_attachment(#[case] attach: Attach) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let args = ["--json", "--color", "always", "--verbose", "generate"];
    let env = [("TERM", "xterm-256color")];
    let output = workspace
        .run_pty(&args, &env, attach)
        .context("run JSON output on a pseudo-terminal")?;
    ensure!(output.status.success(), "JSON generate should succeed");

    let (result_stream, diagnostic_stream) = stream_roles(attach);
    assert_result_document(stream_bytes(&output, result_stream), "generate")?;

    let diagnostics = visible_text(stream_bytes(&output, diagnostic_stream));
    expect_that!(diagnostics.as_str(), not(contains_substring(STAGE_LABEL)));
    expect_that!(diagnostics.as_str(), not(contains_substring("TRACE")));
    expect_that!(diagnostics.as_str(), not(contains_substring("DEBUG")));
    expect_that!(diagnostics.trim().is_empty(), eq(true));

    expect_that!(classify(&output.terminal).has_any_escape, eq(false));
    expect_that!(classify(&output.piped).has_any_escape, eq(false));
    Ok(())
}

/// Show the same fixture under human mode emitting what JSON mode suppresses.
///
/// Without this the JSON assertions could pass on a fixture that produces
/// nothing either way: "stderr is empty" proves suppression only if something
/// would otherwise be written there. The command differs from the JSON case in
/// the two flags under test and in nothing else, so the difference in output is
/// attributable to JSON mode rather than to a different run.
///
/// The stage label is named explicitly because it is the marker an operator
/// would see; a tracing marker is asserted separately so a failure says which
/// mechanism leaked.
#[googletest::gtest]
#[test]
fn human_mode_contrast() -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let args = ["--color", "always", "--verbose", "generate"];
    let env = [("TERM", "xterm-256color")];
    let output = workspace
        .run_pty(&args, &env, Attach::StderrOnly)
        .context("run human-mode contrast on a pseudo-terminal")?;
    ensure!(output.status.success(), "human generate should succeed");

    // Normalise the Fluent bidi isolates first: the stage reporter wraps each
    // number in them, so the literal label never appears in the raw bytes and a
    // substring search would report every stage as absent.
    let diagnostics = visible_text(&output.terminal);
    expect_that!(diagnostics.as_str(), contains_substring(STAGE_LABEL));
    expect_that!(
        diagnostics.contains("TRACE") || diagnostics.contains("DEBUG"),
        eq(true),
        "verbose human mode should emit tracing output on stderr, got {:?}",
        diagnostics
    );
    Ok(())
}
