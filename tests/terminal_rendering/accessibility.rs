//! Check accessible mode decisions with piped output.

use super::support::{
    STAGE_LABELS, TerminalWorkspace, display_plan_for_args, stage_labels_in_output_order,
    visible_text,
};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use pretty_assertions::assert_eq;
use rstest::rstest;
use test_support::terminal::classify;

#[googletest::gtest]
#[rstest]
#[case::auto_no_color("auto", None, &[("NO_COLOR", "1")])]
#[case::auto_dumb_terminal("auto", None, &[("TERM", "dumb")])]
#[case::auto_color_never("auto", Some("never"), &[])]
fn piped_accessibility_auto_reports_static_stage_summaries(
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

    let plan = display_plan_for_args(&args, extra_env, false, false)?;
    let output = workspace
        .run_piped(&args, extra_env)
        .context("run piped accessibility-policy case")?;
    ensure!(output.status.success(), "generate should succeed");

    let stderr = visible_text(&output.stderr);
    let facts = classify(&output.stderr);
    ensure!(
        plan.output_mode().is_accessible(),
        "case should resolve to accessible output"
    );
    assert_eq!(
        stage_labels_in_output_order(&stderr).as_slice(),
        STAGE_LABELS.as_slice(),
    );
    expect_that!(facts.has_redraw, eq(plan.live_progress()));
    // The accessible reporter is the one fixed-output emitter that never emits
    // SGR: it renders a prefix glyph plus a localized label and nothing else.
    // `INV-COLOUR-ADDITIVE` therefore holds for it by construction, and the
    // assertion that keeps that true is "no escape at all", which is stronger
    // than "stripping SGR changes nothing".
    expect_that!(facts.has_any_escape, eq(false));
    expect_that!(facts.has_bare_carriage_return, eq(false));
    Ok(())
}
