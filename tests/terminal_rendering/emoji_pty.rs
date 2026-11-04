//! Check emoji policy combinations on a Unix terminal.

use super::support::{TerminalWorkspace, display_plan_for_args, visible_text};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use rstest::rstest;
use test_support::terminal::{Attach, classify};

#[googletest::gtest]
#[rstest]
#[case::never_standard("never", "off")]
#[case::never_accessible("never", "on")]
#[case::always_standard("always", "off")]
#[case::always_accessible("always", "on")]
#[case::auto_standard("auto", "off")]
#[case::auto_accessible("auto", "on")]
fn terminal_status_glyph_matches_emoji_and_accessibility_policy(
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
    let env = [("TERM", "xterm-256color")];
    let plan = display_plan_for_args(&args, &env, true, true)?;
    let output = workspace
        .run_pty(&args, &env, Attach::Both)
        .context("run terminal emoji-policy case")?;
    ensure!(output.status.success(), "generate should succeed");

    let terminal = visible_text(&output.terminal);
    let expected_prefix = plan.prefs().success_prefix();
    expect_that!(
        terminal.as_str(),
        contains_substring(expected_prefix.as_str())
    );
    expect_that!(terminal.as_str(), contains_substring("Success:"));
    expect_that!(
        classify(&output.terminal).has_non_ascii_glyphs,
        eq(plan.prefs().emoji_allowed()),
    );
    Ok(())
}
