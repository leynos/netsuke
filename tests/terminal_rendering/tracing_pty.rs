//! Pin verbose tracing colour to the resolved colour policy on a Unix terminal.

use super::support::{TerminalWorkspace, display_plan_for_args};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use rstest::rstest;
use test_support::terminal::{Attach, classify};

/// Check that `NO_COLOR` and the TTY facts control verbose PTY logs.
#[googletest::gtest]
#[rstest]
#[case::auto_on_terminal(&[], true)]
#[case::no_color_on_terminal(&[("NO_COLOR", "1")], false)]
fn terminal_verbose_tracing_follows_the_display_plan(
    #[case] extra_env: &[(&str, &str)],
    #[case] expected_colour: bool,
) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let args = ["--color", "auto", "--verbose", "generate"];
    let plan = display_plan_for_args(&args, extra_env, false, true)?;
    let output = workspace
        .run_pty(&args, extra_env, Attach::StderrOnly)
        .context("run PTY verbose tracing case")?;
    ensure!(output.status.success(), "generate should succeed");

    expect_that!(plan.stderr_colour(), eq(expected_colour));
    expect_that!(classify(&output.terminal).has_sgr, eq(expected_colour));
    expect_that!(output.terminal.is_empty(), eq(false));
    Ok(())
}
