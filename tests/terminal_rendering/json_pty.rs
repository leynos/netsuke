//! Verify JSON output stays plain when attached to a Unix pseudo-terminal.

use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use rstest::rstest;
use test_support::terminal::{Attach, classify};

use super::support::{TerminalWorkspace, display_plan_for_args};

#[googletest::gtest]
#[rstest]
#[case::stdout_terminal(Attach::StdoutOnly)]
#[case::stderr_terminal(Attach::StderrOnly)]
fn json_mode_has_no_escape_bytes_on_either_stream(#[case] attach: Attach) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let args = ["--json", "--color", "always", "generate"];
    let env = [("TERM", "xterm-256color")];
    let (stdout_is_terminal, stderr_is_terminal) = match attach {
        Attach::Both => (true, true),
        Attach::StdoutOnly => (true, false),
        Attach::StderrOnly => (false, true),
    };
    let plan = display_plan_for_args(&args, &env, stdout_is_terminal, stderr_is_terminal)?;
    let output = workspace
        .run_pty(&args, &env, attach)
        .context("run JSON output on a pseudo-terminal")?;
    ensure!(output.status.success(), "JSON generate should succeed");
    ensure!(
        !plan.diagnostic_style().colour,
        "JSON diagnostics should resolve without colour"
    );

    expect_that!(classify(&output.terminal).has_any_escape, eq(false));
    expect_that!(classify(&output.piped).has_any_escape, eq(false));
    Ok(())
}
