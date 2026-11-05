//! Step definitions for terminal rendering policy scenarios.

use crate::bdd::fixtures::TestWorld;
use crate::bdd::helpers::assertions::normalize_fluent_isolates;
use anyhow::{Context, Result, ensure};
use rstest_bdd_macros::{then, when};
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

/// Assert that captured stderr contains a Select Graphic Rendition sequence.
#[then("stderr should contain an SGR sequence")]
fn stderr_contains_sgr(world: &TestWorld) -> Result<()> {
    let stderr = world
        .command_stderr
        .get()
        .context("stderr output should be captured")?;
    ensure!(
        classify(stderr.as_bytes()).has_sgr,
        "expected stderr to contain an SGR sequence, got:\n{stderr}"
    );
    Ok(())
}

/// Assert that captured stderr contains no Select Graphic Rendition sequence.
#[then("stderr should contain no SGR sequence")]
fn stderr_contains_no_sgr(world: &TestWorld) -> Result<()> {
    let stderr = world
        .command_stderr
        .get()
        .context("stderr output should be captured")?;
    ensure!(
        !classify(stderr.as_bytes()).has_sgr,
        "expected stderr to contain no SGR sequence, got:\n{stderr}"
    );
    Ok(())
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
    let output = world
        .command_terminal
        .get()
        .context("terminal output should be captured")?;
    ensure!(
        classify(output.as_bytes()).has_sgr,
        "expected terminal output to contain an SGR sequence, got:\n{output}"
    );
    Ok(())
}

/// Assert that captured pseudo-terminal output contains no SGR sequence.
///
/// Paired with a positive text fragment in every scenario that uses it: a
/// terminal that produced nothing at all also contains no SGR, and only the
/// fragment distinguishes "plain" from "silent".
#[then("the terminal output should contain no SGR sequence")]
fn terminal_output_contains_no_sgr(world: &TestWorld) -> Result<()> {
    let output = world
        .command_terminal
        .get()
        .context("terminal output should be captured")?;
    ensure!(
        !classify(output.as_bytes()).has_sgr,
        "expected no SGR sequence in terminal output, got:\n{output}"
    );
    Ok(())
}

/// Assert that captured pseudo-terminal output contains a redraw sequence.
#[then("the terminal output should contain a redraw sequence")]
fn terminal_output_contains_redraw(world: &TestWorld) -> Result<()> {
    let output = world
        .command_terminal
        .get()
        .context("terminal output should be captured")?;
    ensure!(
        classify(output.as_bytes()).has_redraw,
        "expected terminal output to contain a redraw sequence, got:\n{output}"
    );
    Ok(())
}

/// Assert that captured pseudo-terminal output contains no redraw controls.
#[then("the terminal output should contain no redraw sequence")]
fn terminal_output_contains_no_redraw(world: &TestWorld) -> Result<()> {
    let output = world
        .command_terminal
        .get()
        .context("terminal output should be captured")?;
    ensure!(
        !classify(output.as_bytes()).has_redraw,
        "expected no terminal redraw sequence, got:\n{output}"
    );
    Ok(())
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
