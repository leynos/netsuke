//! Check redraw and task-text behaviour with a Unix pseudo-terminal.

use super::support::{STAGE_LABELS, TerminalWorkspace, stage_labels_in_output_order, visible_text};
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
    let output = workspace
        .run_pty(
            &[
                "--accessibility",
                "off",
                "--progress",
                progress_policy,
                "generate",
            ],
            &[("TERM", "xterm-256color")],
            Attach::StderrOnly,
        )
        .context("run terminal progress-policy case")?;
    ensure!(output.status.success(), "generate should succeed");

    let terminal = visible_text(&output.terminal);
    expect_that!(classify(&output.terminal).has_redraw, eq(true));
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
    let output = workspace
        .run_pty(
            &["--accessibility", "off", "--progress", "never", "generate"],
            &[("TERM", "xterm-256color")],
            Attach::StderrOnly,
        )
        .context("run terminal progress-never case")?;
    ensure!(output.status.success(), "generate should succeed");

    let terminal = visible_text(&output.terminal);
    expect_that!(terminal.as_str(), not(contains_substring("Stage 1/6")));
    expect_that!(classify(&output.terminal).has_redraw, eq(false));
    Ok(())
}

#[googletest::gtest]
#[test]
fn task_updates_stay_plain_when_stdout_is_piped_and_stderr_is_terminal() -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let output = workspace
        .run_pty(
            &["--accessibility", "off", "--progress", "always", "build"],
            &[("TERM", "xterm-256color")],
            Attach::StderrOnly,
        )
        .context("run task updates with piped stdout")?;
    ensure!(output.status.success(), "build should succeed");

    let terminal = visible_text(&output.terminal);
    expect_that!(terminal.as_str(), contains_substring("Task 1/2"));
    expect_that!(terminal.as_str(), contains_substring("Task 2/2"));
    let task_lines = output
        .terminal
        .windows(b"Task ".len())
        .enumerate()
        .filter_map(|(start, window)| {
            if window != b"Task " {
                return None;
            }
            let remainder = output.terminal.get(start..)?;
            let line_length = remainder
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(remainder.len(), |newline| newline + 1);
            remainder.get(..line_length)
        })
        .collect::<Vec<_>>();
    expect_that!(task_lines.len(), eq(2));
    for line in task_lines {
        expect_that!(classify(line).has_any_escape, eq(false));
        expect_that!(classify(line).has_redraw, eq(false));
    }
    Ok(())
}
