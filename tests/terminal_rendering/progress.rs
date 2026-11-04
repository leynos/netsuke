//! Check progress summaries when both Netsuke output streams are piped.

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
#[case::never_suppresses_stages("never")]
#[case::auto_reports_static_stages("auto")]
#[case::always_reports_static_stages("always")]
fn piped_progress_policy_controls_stage_summaries(#[case] progress_policy: &str) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let args = [
        "--accessibility",
        "off",
        "--progress",
        progress_policy,
        "generate",
    ];
    let plan = display_plan_for_args(&args, &[], false, false)?;
    let output = workspace
        .run_piped(&args, &[])
        .context("run piped progress-policy case")?;
    ensure!(output.status.success(), "generate should succeed");

    let stderr = visible_text(&output.stderr);
    let facts = classify(&output.stderr);
    expect_that!(facts.has_redraw, eq(plan.live_progress()));
    if plan.progress_enabled() {
        assert_eq!(
            stage_labels_in_output_order(&stderr).as_slice(),
            STAGE_LABELS.as_slice(),
        );
    } else {
        expect_that!(stderr.as_str(), not(contains_substring("Stage 1/6")));
    }
    Ok(())
}
