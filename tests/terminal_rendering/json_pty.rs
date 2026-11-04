//! Verify JSON output stays plain when attached to a Unix pseudo-terminal.

use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use rstest::rstest;
use test_support::terminal::{Attach, classify};

use super::support::TerminalWorkspace;

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
