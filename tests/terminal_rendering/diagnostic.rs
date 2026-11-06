//! Check that Miette diagnostics follow the resolved colour and glyph policy.

use super::support::{TerminalWorkspace, display_plan_for_args, visible_text};
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

/// Check that forced colour styles a diagnostic written to a pipe.
#[googletest::gtest]
#[test]
fn piped_diagnostic_colour_matches_the_display_plan() -> Result<()> {
    let workspace = empty_workspace()?;
    let args = ["--color", "always", "generate"];
    let plan = display_plan_for_args(&args, &[], false, false)?;
    let output = workspace
        .run_piped(&args, &[])
        .context("run piped missing-manifest diagnostic")?;
    ensure!(!output.status.success(), "missing manifest should fail");

    expect_that!(plan.diagnostic_style().colour, eq(true));
    expect_that!(classify(&output.stderr).has_sgr, eq(true));
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

/// Create a terminal workspace without its manifest for diagnostic cases.
fn empty_workspace() -> Result<TerminalWorkspace> {
    let workspace = TerminalWorkspace::new()?;
    test_support::fs::remove_file(workspace.root_path().join("Netsukefile"))
        .context("remove the staged manifest")?;
    Ok(workspace)
}
