//! Check Miette diagnostic colour and glyphs on a Unix pseudo-terminal.
//!
//! Every case here needs the Unix-only PTY harness. The pipe cases live in
//! `diagnostic`, which is compiled on every platform.

use super::support::{display_plan_for_args, empty_workspace, visible_text};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use rstest::rstest;
use test_support::terminal::{Attach, classify};

/// Check colour on Miette diagnostics written to a pseudo-terminal.
#[googletest::gtest]
#[rstest]
#[case::never(&["--color", "never", "generate"], &[], false)]
#[case::no_color(&["--color", "auto", "generate"], &[("NO_COLOR", "1")], false)]
#[case::accessible(
    &["--color", "always", "--accessibility", "on", "generate"],
    &[],
    true
)]
fn terminal_diagnostic_colour_matches_the_display_plan(
    #[case] args: &[&str],
    #[case] extra_env: &[(&str, &str)],
    #[case] expected_colour: bool,
) -> Result<()> {
    let workspace = empty_workspace()?;
    let plan = display_plan_for_args(args, extra_env, false, true)?;
    let output = workspace
        .run_pty(args, extra_env, Attach::StderrOnly)
        .context("run missing-manifest diagnostic on a pseudo-terminal")?;
    ensure!(!output.status.success(), "missing manifest should fail");

    let style = plan.diagnostic_style();
    expect_that!(style.colour, eq(expected_colour));
    expect_that!(classify(&output.terminal).has_sgr, eq(expected_colour));
    if plan.output_mode().is_accessible() {
        expect_that!(style.links, eq(false));
        expect_that!(classify(&output.terminal).has_osc, eq(false));
    }
    expect_that!(
        visible_text(&output.terminal).as_str(),
        contains_substring("Netsukefile")
    );
    Ok(())
}

/// Check that Miette uses the plan's ASCII setting when emoji are disabled.
#[googletest::gtest]
#[test]
fn diagnostic_emoji_never_selects_ascii_glyphs() -> Result<()> {
    let workspace = empty_workspace()?;
    let args = ["--color", "always", "--emoji", "never", "generate"];
    let plan = display_plan_for_args(&args, &[], false, true)?;
    let output = workspace
        .run_pty(&args, &[], Attach::StderrOnly)
        .context("run ASCII missing-manifest diagnostic")?;
    ensure!(!output.status.success(), "missing manifest should fail");

    expect_that!(plan.diagnostic_style().unicode, eq(false));
    expect_that!(
        classify(visible_text(&output.terminal).as_bytes()).has_non_ascii_glyphs,
        eq(false)
    );
    Ok(())
}
