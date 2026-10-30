//! Check progress summaries when both Netsuke output streams are piped.

use super::support::{STAGE_LABELS, TerminalWorkspace, stage_labels_in_output_order, visible_text};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use pretty_assertions::assert_eq;
use rstest::rstest;
use test_support::terminal::classify;

#[googletest::gtest]
#[rstest]
#[case::never_suppresses_stages("never", false)]
#[case::auto_reports_static_stages("auto", true)]
#[case::always_reports_static_stages("always", true)]
fn piped_progress_policy_controls_stage_summaries(
    #[case] progress_policy: &str,
    #[case] should_report_stages: bool,
) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let output = workspace
        .run_piped(
            &[
                "--accessibility",
                "off",
                "--progress",
                progress_policy,
                "generate",
            ],
            &[],
        )
        .context("run piped progress-policy case")?;
    ensure!(output.status.success(), "generate should succeed");

    let stderr = visible_text(&output.stderr);
    let facts = classify(&output.stderr);
    expect_that!(facts.has_redraw, eq(false));
    if should_report_stages {
        assert_eq!(
            stage_labels_in_output_order(&stderr).as_slice(),
            STAGE_LABELS.as_slice(),
        );
    } else {
        expect_that!(stderr.as_str(), not(contains_substring("Stage 1/6")));
    }
    Ok(())
}
