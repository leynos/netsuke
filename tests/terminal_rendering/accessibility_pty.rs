//! Check accessible and standard progress decisions on Unix pseudo-terminals.

use super::support::{STAGE_LABELS, TerminalWorkspace, stage_labels_in_output_order, visible_text};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use pretty_assertions::assert_eq;
use rstest::rstest;
use test_support::terminal::{Attach, classify};

#[googletest::gtest]
#[rstest]
#[case::on_is_static("on", None, &[("TERM", "xterm-256color")], false)]
#[case::off_ignores_no_color("off", None, &[("NO_COLOR", "1"), ("TERM", "xterm-256color")], true)]
#[case::off_with_dumb_term_is_static("off", None, &[("TERM", "dumb")], false)]
#[case::auto_with_no_color_is_static("auto", None, &[("NO_COLOR", "1"), ("TERM", "xterm-256color")], false)]
#[case::auto_with_dumb_term_is_static("auto", None, &[("TERM", "dumb")], false)]
#[case::auto_with_color_never_is_static("auto", Some("never"), &[("TERM", "xterm-256color")], false)]
fn terminal_accessibility_policy_controls_redraw(
    #[case] accessibility: &str,
    #[case] color: Option<&str>,
    #[case] extra_env: &[(&str, &str)],
    #[case] should_redraw: bool,
) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let mut args = vec!["--accessibility", accessibility, "--progress", "always"];
    if let Some(color_policy) = color {
        args.extend(["--color", color_policy]);
    }
    args.push("generate");

    let output = workspace
        .run_pty(&args, extra_env, Attach::StderrOnly)
        .context("run terminal accessibility-policy case")?;
    ensure!(output.status.success(), "generate should succeed");

    let terminal = visible_text(&output.terminal);
    expect_that!(classify(&output.terminal).has_redraw, eq(should_redraw));
    if should_redraw {
        expect_that!(terminal.as_str(), contains_substring("Stage 1/6"));
    } else {
        assert_eq!(
            stage_labels_in_output_order(&terminal).as_slice(),
            STAGE_LABELS.as_slice(),
        );
    }
    Ok(())
}
