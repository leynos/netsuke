//! Check accessible and standard progress decisions on Unix pseudo-terminals.

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
#[case::on_is_static("on", None, &[("TERM", "xterm-256color")])]
#[case::off_ignores_no_color("off", None, &[("NO_COLOR", "1"), ("TERM", "xterm-256color")])]
#[case::off_with_dumb_term_is_static("off", None, &[("TERM", "dumb")])]
#[case::auto_with_no_color_is_static("auto", None, &[("NO_COLOR", "1"), ("TERM", "xterm-256color")])]
#[case::auto_with_dumb_term_is_static("auto", None, &[("TERM", "dumb")])]
#[case::auto_with_color_never_is_static("auto", Some("never"), &[("TERM", "xterm-256color")])]
fn terminal_accessibility_policy_controls_redraw(
    #[case] accessibility: &str,
    #[case] color: Option<&str>,
    #[case] extra_env: &[(&str, &str)],
) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let mut args = vec!["--accessibility", accessibility, "--progress", "always"];
    if let Some(color_policy) = color {
        args.extend(["--color", color_policy]);
    }
    args.push("generate");

    let plan = display_plan_for_args(&args, extra_env, false, true)?;
    let output = workspace
        .run_pty(&args, extra_env, Attach::StderrOnly)
        .context("run terminal accessibility-policy case")?;
    ensure!(output.status.success(), "generate should succeed");

    let terminal = visible_text(&output.terminal);
    expect_that!(
        classify(&output.terminal).has_redraw,
        eq(plan.live_progress())
    );
    if plan.live_progress() {
        expect_that!(terminal.as_str(), contains_substring("Stage 1/6"));
    } else {
        assert_eq!(
            stage_labels_in_output_order(&terminal).as_slice(),
            STAGE_LABELS.as_slice(),
        );
    }
    Ok(())
}
