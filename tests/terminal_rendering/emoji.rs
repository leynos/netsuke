//! Check emoji policy combinations when Netsuke writes to pipes.

use super::support::{TerminalWorkspace, visible_text};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use rstest::rstest;
use test_support::terminal::classify;

#[googletest::gtest]
#[rstest]
#[case::never_standard("never", "off", "+ Success:", false)]
#[case::never_accessible("never", "on", "+ Success:", false)]
#[case::always_standard("always", "off", "✔ Success:", true)]
#[case::always_accessible("always", "on", "✔ Success:", true)]
#[case::auto_standard("auto", "off", "✔ Success:", true)]
#[case::auto_accessible("auto", "on", "+ Success:", false)]
fn piped_status_glyph_matches_emoji_and_accessibility_policy(
    #[case] emoji_policy: &str,
    #[case] accessibility: &str,
    #[case] expected_prefix: &str,
    #[case] expected_unicode: bool,
) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let output = workspace
        .run_piped(
            &[
                "--emoji",
                emoji_policy,
                "--accessibility",
                accessibility,
                "--progress",
                "always",
                "generate",
            ],
            &[],
        )
        .context("run piped emoji-policy case")?;
    ensure!(output.status.success(), "generate should succeed");

    let stderr = visible_text(&output.stderr);
    expect_that!(stderr.as_str(), contains_substring(expected_prefix));
    expect_that!(stderr.as_str(), contains_substring("Success:"));
    expect_that!(
        classify(&output.stderr).has_non_ascii_glyphs,
        eq(expected_unicode),
    );
    Ok(())
}
