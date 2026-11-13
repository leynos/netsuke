//! Check emoji policy combinations when Netsuke writes to pipes.

use super::support::{TerminalWorkspace, display_plan_for_args, visible_text};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use rstest::rstest;
use test_support::terminal::classify;

#[googletest::gtest]
#[rstest]
#[case::never_standard("never", "off")]
#[case::never_accessible("never", "on")]
#[case::always_standard("always", "off")]
#[case::always_accessible("always", "on")]
#[case::auto_standard("auto", "off")]
#[case::auto_accessible("auto", "on")]
fn piped_status_glyph_matches_emoji_and_accessibility_policy(
    #[case] emoji_policy: &str,
    #[case] accessibility: &str,
) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let args = [
        "--emoji",
        emoji_policy,
        "--accessibility",
        accessibility,
        "--progress",
        "always",
        "generate",
    ];
    let plan = display_plan_for_args(&args, &[], false, false)?;
    let output = workspace
        .run_piped(&args, &[])
        .context("run piped emoji-policy case")?;
    ensure!(output.status.success(), "generate should succeed");

    let stderr = visible_text(&output.stderr);
    let expected_prefix = plan.prefs().success_prefix();
    expect_that!(
        stderr.as_str(),
        contains_substring(expected_prefix.as_str())
    );
    expect_that!(stderr.as_str(), contains_substring("Success:"));
    expect_that!(
        classify(&output.stderr).has_non_ascii_glyphs,
        eq(plan.prefs().emoji_allowed()),
    );
    Ok(())
}
