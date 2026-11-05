//! Pin verbose tracing colour to the resolved colour policy and stream facts.

use super::support::{TerminalWorkspace, display_plan_for_args};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use rstest::rstest;
use test_support::terminal::{Attach, classify};

/// Check colour policy for verbose logs captured through pipes.
#[googletest::gtest]
#[rstest]
#[case::never("never", false)]
#[case::always("always", true)]
fn piped_verbose_tracing_follows_the_display_plan(
    #[case] colour: &str,
    #[case] expected_colour: bool,
) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let args = ["--color", colour, "--verbose", "generate"];
    let plan = display_plan_for_args(&args, &[], false, false)?;
    let output = workspace
        .run_piped(&args, &[])
        .context("run piped verbose tracing case")?;
    ensure!(output.status.success(), "generate should succeed");

    expect_that!(plan.stderr_colour(), eq(expected_colour));
    expect_that!(classify(&output.stderr).has_sgr, eq(expected_colour));
    expect_that!(output.stderr.is_empty(), eq(false));
    Ok(())
}

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

/// Check colour policy on the tracing event emitted for a non-runner failure.
#[googletest::gtest]
#[rstest]
#[case::never("never", false)]
#[case::always("always", true)]
fn tracing_failure_color_follows_the_display_plan(
    #[case] colour: &str,
    #[case] expected_colour: bool,
) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let cycle_manifest =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/circular.yml");
    test_support::fs::copy(cycle_manifest, workspace.root_path().join("Netsukefile"))
        .context("stage circular manifest")?;

    let args = ["--color", colour, "generate"];
    let plan = display_plan_for_args(&args, &[], false, false)?;
    let output = workspace
        .run_piped(&args, &[])
        .context("run circular-manifest tracing case")?;
    ensure!(!output.status.success(), "circular manifest should fail");
    expect_that!(
        String::from_utf8_lossy(&output.stderr).as_ref(),
        contains_substring("runner failed")
    );
    expect_that!(plan.stderr_colour(), eq(expected_colour));
    expect_that!(classify(&output.stderr).has_sgr, eq(expected_colour));
    Ok(())
}
