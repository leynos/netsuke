//! Check emoji policy combinations on a Unix terminal.

use super::support::{TerminalWorkspace, visible_text};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use rstest::rstest;
use test_support::terminal::{Attach, classify};

#[googletest::gtest]
#[rstest]
#[case::never_standard("never", "off", "+ Success:", false)]
#[case::never_accessible("never", "on", "+ Success:", false)]
#[case::always_standard("always", "off", "✔ Success:", true)]
#[case::always_accessible("always", "on", "✔ Success:", true)]
#[case::auto_standard("auto", "off", "✔ Success:", true)]
#[case::auto_accessible("auto", "on", "+ Success:", false)]
fn terminal_status_glyph_matches_emoji_and_accessibility_policy(
    #[case] emoji_policy: &str,
    #[case] accessibility: &str,
    #[case] expected_prefix: &str,
    #[case] expected_unicode: bool,
) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let output = workspace
        .run_pty(
            &[
                "--emoji",
                emoji_policy,
                "--accessibility",
                accessibility,
                "--progress",
                "always",
                "generate",
            ],
            &[("TERM", "xterm-256color")],
            Attach::Both,
        )
        .context("run terminal emoji-policy case")?;
    ensure!(output.status.success(), "generate should succeed");

    let terminal = visible_text(&output.terminal);
    expect_that!(terminal.as_str(), contains_substring(expected_prefix));
    expect_that!(terminal.as_str(), contains_substring("Success:"));
    expect_that!(
        classify(&output.terminal).has_non_ascii_glyphs,
        eq(expected_unicode),
    );
    Ok(())
}
