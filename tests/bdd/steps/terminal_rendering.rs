//! Step definitions for terminal rendering policy scenarios.

use crate::bdd::fixtures::TestWorld;
use crate::bdd::helpers::assertions::normalize_fluent_isolates;
use anyhow::{Context, Result, ensure};
use rstest_bdd_macros::then;
// `when` is imported only where it is used: the step it annotates needs the
// Unix-only PTY harness, so on other platforms the import is dead and
// `-D warnings` rejects it.
#[cfg(unix)]
use rstest_bdd_macros::when;
#[cfg(unix)]
use std::time::Duration;
use test_support::terminal::classify;
#[cfg(unix)]
use test_support::terminal::{Attach, PtyRequest, run_in_pty};

#[cfg(unix)]
use super::manifest_command::manifest_command_helpers::build_netsuke_process;

/// Run Netsuke with selected output streams attached to a pseudo-terminal.
#[cfg(unix)]
#[when("netsuke is run on a terminal with arguments {args:string}")]
fn run_netsuke_on_terminal(world: &TestWorld, args: &str) -> Result<()> {
    let argv: Vec<_> = args.split_whitespace().collect();
    let command = build_netsuke_process(world, &argv)?;
    let output = run_in_pty(PtyRequest {
        command,
        attach: Attach::Both,
        timeout: Duration::from_secs(15),
    })
    .context("run Netsuke on a pseudo-terminal")?;
    let terminal = String::from_utf8_lossy(&output.terminal).into_owned();
    world.command_terminal.set(terminal.clone());
    world.run_status.set(output.status.success());
    if output.status.success() {
        world.run_error.clear();
    } else {
        world.run_error.set(terminal);
    }
    Ok(())
}

/// Which captured stream a classifier assertion reads.
#[derive(Clone, Copy)]
enum Stream {
    /// The captured stderr of a piped run.
    Stderr,
    /// The captured output of a pseudo-terminal run.
    Terminal,
}

/// One classifier fact an assertion asserts and the message describing it.
struct ClassifierFact {
    /// The classifier predicate the captured bytes are tested with.
    observed: fn(&[u8]) -> bool,
    /// The expectation, phrased so it reads after "expected {subject} to".
    expectation: &'static str,
}

/// Return whether the bytes contain a Select Graphic Rendition sequence.
fn has_sgr(bytes: &[u8]) -> bool {
    classify(bytes).has_sgr
}

/// Return whether the bytes contain no Select Graphic Rendition sequence.
fn has_no_sgr(bytes: &[u8]) -> bool {
    !has_sgr(bytes)
}

/// Return whether the bytes contain a redraw sequence.
fn has_redraw(bytes: &[u8]) -> bool {
    classify(bytes).has_redraw
}

/// Return whether the bytes contain no redraw sequence.
fn has_no_redraw(bytes: &[u8]) -> bool {
    !has_redraw(bytes)
}

/// The SGR fact, asserted present or absent.
const SGR: [ClassifierFact; 2] = [
    ClassifierFact {
        observed: has_sgr,
        expectation: "contain an SGR sequence",
    },
    ClassifierFact {
        observed: has_no_sgr,
        expectation: "contain no SGR sequence",
    },
];

/// The redraw fact, asserted present or absent.
const REDRAW: [ClassifierFact; 2] = [
    ClassifierFact {
        observed: has_redraw,
        expectation: "contain a redraw sequence",
    },
    ClassifierFact {
        observed: has_no_redraw,
        expectation: "contain no redraw sequence",
    },
];

/// Assert a classifier fact over one captured stream.
///
/// Every absence assertion is paired with a positive text fragment in the
/// scenario that uses it: a stream that produced nothing at all also contains
/// no SGR sequence, and only the fragment distinguishes "plain" from "silent".
fn assert_classifier_fact(world: &TestWorld, stream: Stream, fact: &ClassifierFact) -> Result<()> {
    let (subject, captured) = match stream {
        Stream::Stderr => ("stderr", world.command_stderr.get()),
        Stream::Terminal => ("terminal output", world.command_terminal.get()),
    };
    let output = captured.with_context(|| format!("{subject} should be captured"))?;
    ensure!(
        (fact.observed)(output.as_bytes()),
        "expected {subject} to {}, got:\n{output}",
        fact.expectation
    );
    Ok(())
}

/// Assert that captured stderr contains a Select Graphic Rendition sequence.
#[then("stderr should contain an SGR sequence")]
fn stderr_contains_sgr(world: &TestWorld) -> Result<()> {
    assert_classifier_fact(world, Stream::Stderr, &SGR[0])
}

/// Assert that captured stderr contains no Select Graphic Rendition sequence.
#[then("stderr should contain no SGR sequence")]
fn stderr_contains_no_sgr(world: &TestWorld) -> Result<()> {
    assert_classifier_fact(world, Stream::Stderr, &SGR[1])
}

/// Assert that status glyphs use the selected ASCII or Unicode set.
#[then("stderr status glyphs should be {glyphs}")]
fn stderr_status_glyphs_match(world: &TestWorld, glyphs: &str) -> Result<()> {
    let stderr = world
        .command_stderr
        .get()
        .context("stderr output should be captured")?;
    let normalized = normalize_fluent_isolates(&stderr);
    let has_unicode = classify(normalized.as_bytes()).has_non_ascii_glyphs;
    match glyphs {
        "ASCII" => ensure!(!has_unicode, "expected ASCII status glyphs, got:\n{stderr}"),
        "Unicode" => ensure!(
            has_unicode,
            "expected Unicode status glyphs, got:\n{stderr}"
        ),
        other => anyhow::bail!("unsupported status glyph set: {other}"),
    }
    Ok(())
}

/// Assert that every non-empty status line includes its textual stage or result.
#[then("every stderr status line should carry a text label")]
fn stderr_status_lines_have_text_labels(world: &TestWorld) -> Result<()> {
    let stderr = world
        .command_stderr
        .get()
        .context("stderr output should be captured")?;
    let normalized = normalize_fluent_isolates(&stderr);
    let lines: Vec<_> = normalized
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    ensure!(!lines.is_empty(), "expected status output on stderr");
    for line in lines {
        ensure!(
            line.contains("Stage ") || line.contains("Success:"),
            "status line has no textual stage or result label: {line:?}"
        );
    }
    Ok(())
}

/// Assert that captured pseudo-terminal output contains an SGR sequence.
#[then("the terminal output should contain an SGR sequence")]
fn terminal_output_contains_sgr(world: &TestWorld) -> Result<()> {
    assert_classifier_fact(world, Stream::Terminal, &SGR[0])
}

/// Assert that captured pseudo-terminal output contains no SGR sequence.
#[then("the terminal output should contain no SGR sequence")]
fn terminal_output_contains_no_sgr(world: &TestWorld) -> Result<()> {
    assert_classifier_fact(world, Stream::Terminal, &SGR[1])
}

/// Assert that captured pseudo-terminal output contains a redraw sequence.
#[then("the terminal output should contain a redraw sequence")]
fn terminal_output_contains_redraw(world: &TestWorld) -> Result<()> {
    assert_classifier_fact(world, Stream::Terminal, &REDRAW[0])
}

/// Assert that captured pseudo-terminal output contains no redraw controls.
#[then("the terminal output should contain no redraw sequence")]
fn terminal_output_contains_no_redraw(world: &TestWorld) -> Result<()> {
    assert_classifier_fact(world, Stream::Terminal, &REDRAW[1])
}

/// Assert that captured pseudo-terminal output contains the requested text.
#[then("the terminal output should contain {fragment:string}")]
fn terminal_output_contains_text(world: &TestWorld, fragment: &str) -> Result<()> {
    let output = world
        .command_terminal
        .get()
        .context("terminal output should be captured")?;
    let normalized_output = normalize_fluent_isolates(&output);
    let normalized_fragment = normalize_fluent_isolates(fragment);
    ensure!(
        normalized_output.contains(&normalized_fragment),
        "expected terminal output to contain '{fragment}', got:\n{output}"
    );
    Ok(())
}
