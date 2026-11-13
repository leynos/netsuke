//! Pin verbose tracing colour to the resolved colour policy and stream facts.
//!
//! Only the pipe cases live here. The pseudo-terminal case needs the Unix-only
//! PTY harness and is in `tracing_pty`, because this module is compiled on every
//! platform and naming a Unix-gated item in an ungated module fails the
//! Windows build.

use super::support::{TerminalWorkspace, display_plan_for_args};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use rstest::rstest;
use test_support::terminal::classify;

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
