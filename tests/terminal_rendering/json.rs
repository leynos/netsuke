//! Check that JSON output and early configuration errors remain free of escapes.

use super::support::{TerminalWorkspace, visible_text};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use rstest::rstest;
use test_support::{
    fs,
    terminal::{Attach, classify},
};

#[googletest::gtest]
#[rstest]
#[case::stdout_terminal(Attach::StdoutOnly)]
#[case::stderr_terminal(Attach::StderrOnly)]
fn json_mode_has_no_escape_bytes_on_either_stream(#[case] attach: Attach) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let output = workspace
        .run_pty(
            &["--json", "--color", "always", "generate"],
            &[("TERM", "xterm-256color")],
            attach,
        )
        .context("run JSON output on a pseudo-terminal")?;
    ensure!(output.status.success(), "JSON generate should succeed");

    expect_that!(classify(&output.terminal).has_any_escape, eq(false));
    expect_that!(classify(&output.piped).has_any_escape, eq(false));
    Ok(())
}

#[googletest::gtest]
#[test]
fn configuration_load_failure_under_color_always_stays_plain() -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    fs::write(workspace.root_path().join("broken.toml"), "not = [valid\n")
        .context("write malformed configuration")?;
    let output = workspace
        .run_piped(
            &["--config", "broken.toml", "--color", "always", "generate"],
            &[],
        )
        .context("run command with malformed configuration")?;
    ensure!(
        !output.status.success(),
        "malformed configuration should fail before execution"
    );
    expect_that!(
        visible_text(&output.stderr).as_str(),
        contains_substring("broken.toml"),
    );
    expect_that!(classify(&output.stdout).has_any_escape, eq(false));
    expect_that!(classify(&output.stderr).has_any_escape, eq(false));
    Ok(())
}
