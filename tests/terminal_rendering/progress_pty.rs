//! Check redraw and task-text behaviour with a Unix pseudo-terminal.

use super::support::{
    STAGE_LABELS, TerminalWorkspace, display_plan_for_args, stage_labels_in_output_order,
    visible_text,
};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use pretty_assertions::assert_eq;
use rstest::rstest;
use test_support::terminal::{Attach, classify};

#[googletest::gtest]
#[rstest]
#[case::auto_redraws("auto")]
#[case::always_redraws("always")]
fn standard_progress_redraws_on_a_capable_terminal(#[case] progress_policy: &str) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let args = [
        "--accessibility",
        "off",
        "--progress",
        progress_policy,
        "generate",
    ];
    let env = [("TERM", "xterm-256color")];
    let plan = display_plan_for_args(&args, &env, false, true)?;
    let output = workspace
        .run_pty(&args, &env, Attach::StderrOnly)
        .context("run terminal progress-policy case")?;
    ensure!(output.status.success(), "generate should succeed");

    let terminal = visible_text(&output.terminal);
    expect_that!(
        classify(&output.terminal).has_redraw,
        eq(plan.live_progress())
    );
    assert_eq!(
        stage_labels_in_output_order(&terminal).as_slice(),
        STAGE_LABELS.as_slice(),
    );
    Ok(())
}

#[googletest::gtest]
#[test]
fn terminal_progress_never_suppresses_stage_output() -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let args = ["--accessibility", "off", "--progress", "never", "generate"];
    let env = [("TERM", "xterm-256color")];
    let plan = display_plan_for_args(&args, &env, false, true)?;
    let output = workspace
        .run_pty(&args, &env, Attach::StderrOnly)
        .context("run terminal progress-never case")?;
    ensure!(output.status.success(), "generate should succeed");

    let terminal = visible_text(&output.terminal);
    expect_that!(terminal.as_str(), not(contains_substring("Stage 1/6")));
    expect_that!(
        classify(&output.terminal).has_redraw,
        eq(plan.live_progress())
    );
    Ok(())
}

/// Slice out the text of each task update, from its label to its line end.
///
/// A line ends at `\r` as well as `\n`: the progress animation terminates each
/// repaint with a bare carriage return, so only counting line feeds merges the
/// whole capture into one line.
///
/// The return stops at the label rather than at the start of the line because
/// the stage reporter and the task reporter write to the same terminal line.
/// The capture this test reads contains the stage-6 repaint, its padding, and
/// then `Task 1/2` in one line with no terminator between them. Starting the
/// slice at the line start would classify the stage animation's cursor moves
/// as task redraw, so the assertion would fail on evidence the task update
/// never produced. The stage animation is asserted on its own by
/// `standard_progress_redraws_on_a_capable_terminal`.
fn task_lines(bytes: &[u8]) -> Vec<&[u8]> {
    let ends_line = |byte: &u8| matches!(*byte, b'\r' | b'\n');
    bytes
        .windows(b"Task ".len())
        .enumerate()
        .filter_map(|(start, window)| {
            if window != b"Task " {
                return None;
            }
            let remainder = bytes.get(start..)?;
            let line_length = remainder
                .iter()
                .position(ends_line)
                .map_or(remainder.len(), |terminator| terminator + 1);
            let line_end = start.saturating_add(line_length);
            bytes.get(start..line_end)
        })
        .collect()
}

/// A task update is sliced from its label to its line end, nothing earlier.
///
/// This is the shape the real capture has: the stage animation repaints, then
/// the task text follows on the same terminal line with no terminator between
/// them. Slicing from the start of the line would hand the stage redraw to the
/// task assertion; slicing from the label stops at the last possible point and
/// so proves the assertion is about the task text alone.
///
/// The carriage return in the fixture is what ends the *task* update, so a
/// slice that ignored it would run on into the next line.
#[test]
fn a_task_update_is_sliced_from_its_label_to_its_line_end() {
    // The bidi isolates are U+2068 and U+2069: the reporter wraps each number
    // in them so a right-to-left terminal keeps the counts in order.
    let capture =
        "\x1b[2Kstage one\r  \u{2068}Task \u{2068}1\u{2069}/\u{2068}2\u{2069}\u{2069}: x\r\n";
    let expected = "Task \u{2068}1\u{2069}/\u{2068}2\u{2069}\u{2069}: x\r";
    let lines = task_lines(capture.as_bytes());
    assert_eq!(lines.len(), 1, "one task line expected");
    assert_eq!(
        lines.first().copied(),
        Some(expected.as_bytes()),
        "the slice starts at the label and stops after the terminator",
    );
}

/// The UTF-8 encoding of U+2068, the bidi isolate that opens a task update.
const TASK_ISOLATE: [u8; 3] = [0xE2, 0x81, 0xA8];

/// Slice out each task-update emission, isolation prefix included.
///
/// The reporter wraps the whole label in `U+2068`…`U+2069`, so the emission
/// begins one character before the `Task ` text. Slicing at the label would
/// leave that prefix outside the assertion, and an escape written there would
/// pass unseen.
///
/// This is *not* the whole emission. It stops at the isolate, so bytes written
/// before it remain unexamined: an edit that wrote `ESC[31m` ahead of the label
/// would pass every assertion here. The stream cannot settle that case —
/// widening the slice to the previous terminator collects the stage
/// animation's own escapes, because the animation repaints with cursor-up
/// moves and the first update shares its physical line. That is exactly what
/// an earlier revision of this helper did, and it failed on correct output.
///
/// The write-site boundary is therefore covered where it actually exists, by
/// `reporter_text_write_carries_no_escape_from_its_first_byte` in
/// `src/status/tests.rs`, which captures the bytes the reporter writes and so
/// sees the prefix. This helper's job is narrower: prove the emitted line,
/// from its isolate onward, survives the real pseudo-terminal path intact.
fn task_update_emissions(bytes: &[u8]) -> Vec<&[u8]> {
    let is_terminator = |byte: &u8| matches!(*byte, b'\r' | b'\n');
    bytes
        .windows(b"Task ".len())
        .enumerate()
        .filter_map(|(start, window)| {
            if window != b"Task " {
                return None;
            }
            let emission_start = start
                .checked_sub(TASK_ISOLATE.len())
                .filter(|&index| bytes.get(index..start) == Some(TASK_ISOLATE.as_slice()))
                .unwrap_or(start);
            let remainder = bytes.get(emission_start..)?;
            let emission_length = remainder
                .iter()
                .position(is_terminator)
                .map_or(remainder.len(), |terminator| terminator + 1);
            bytes.get(emission_start..emission_start.saturating_add(emission_length))
        })
        .collect()
}

#[googletest::gtest]
#[test]
fn task_updates_stay_plain_when_stdout_is_piped_and_stderr_is_terminal() -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let args = ["--accessibility", "off", "--progress", "always", "build"];
    let env = [("TERM", "xterm-256color")];
    let output = workspace
        .run_pty(&args, &env, Attach::StderrOnly)
        .context("run task updates with piped stdout")?;
    ensure!(output.status.success(), "build should succeed");

    let terminal = visible_text(&output.terminal);
    expect_that!(terminal.as_str(), contains_substring("Task 1/2"));
    expect_that!(terminal.as_str(), contains_substring("Task 2/2"));
    let emissions = task_update_emissions(&output.terminal);
    expect_that!(emissions.len(), eq(2));
    for emission in emissions {
        // Piped stdout puts the reporter on the text path, which writes the
        // update with one `writeln!` and emits no escape at all. This slice
        // starts at the bidi isolate, so it covers the line from the isolate
        // onward; bytes before the isolate are out of reach here and are
        // covered by the write-site capture in `src/status/tests.rs`.
        expect_that!(emission.starts_with(TASK_ISOLATE.as_slice()), eq(true));
        // The slice stops at the first terminator, and on a pseudo-terminal
        // that is the `\r` of the `\r\n` the line discipline writes.
        expect_that!(emission.ends_with(b"\r"), eq(true));
        expect_that!(classify(emission).has_any_escape, eq(false));
        expect_that!(classify(emission).has_redraw, eq(false));
    }
    Ok(())
}
