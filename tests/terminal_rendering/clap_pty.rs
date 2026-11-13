//! Pin Clap help colour on a Unix pseudo-terminal.
//!
//! Every case here needs the Unix-only PTY harness. The pipe cases live in
//! `clap`, which is compiled on every platform.

use super::support::{TerminalWorkspace, display_plan_for_args};
use anyhow::{Context, Result};
use googletest::prelude::*;
use rstest::rstest;
use test_support::terminal::{Attach, classify};

/// Pin Clap help colour on a terminal for every help-rendering route.
#[googletest::gtest]
#[rstest]
#[case::auto_terminal(&["--color", "auto"], &[], &["--help"], true)]
#[case::auto_terminal(&["--color", "auto"], &[], &["help"], true)]
#[case::auto_terminal(&["--color", "auto"], &[], &["help", "build"], true)]
#[case::no_color_terminal(&["--color", "auto"], &[("NO_COLOR", "1")], &["--help"], false)]
#[case::no_color_terminal(&["--color", "auto"], &[("NO_COLOR", "1")], &["help"], false)]
#[case::no_color_terminal(
    &["--color", "auto"],
    &[("NO_COLOR", "1")],
    &["help", "build"],
    false
)]
#[case::dumb_terminal(&["--color", "auto"], &[("TERM", "dumb")], &["--help"], false)]
#[case::dumb_terminal(&["--color", "auto"], &[("TERM", "dumb")], &["help"], false)]
#[case::never_terminal(&["--color", "never"], &[], &["--help"], false)]
#[case::never_terminal(&["--color", "never"], &[], &["help"], false)]
fn terminal_help_colour_matches_the_startup_policy(
    #[case] colour_args: &[&str],
    #[case] extra_env: &[(&str, &str)],
    #[case] help_args: &[&str],
    #[case] expected_colour: bool,
) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let plan = display_plan_for_args(colour_args, extra_env, true, true)?;
    let args = colour_args
        .iter()
        .chain(help_args.iter())
        .copied()
        .collect::<Vec<_>>();
    let output = workspace
        .run_pty(&args, extra_env, Attach::StdoutOnly)
        .with_context(|| format!("run help form {args:?} on a pseudo-terminal"))?;

    expect_that!(output.status.success(), eq(true));
    expect_that!(plan.stdout_colour(), eq(expected_colour));
    expect_that!(classify(&output.terminal).has_sgr, eq(expected_colour));
    expect_that!(
        String::from_utf8_lossy(&output.terminal),
        contains_substring("Usage")
    );
    Ok(())
}

/// Keep invalid-argument usage diagnostics plain even under forced colour.
#[googletest::gtest]
#[test]
fn usage_error_remains_plain_when_colour_is_forced() -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let output = workspace
        .run_pty(
            &["--color", "always", "unknown-command"],
            &[],
            Attach::StderrOnly,
        )
        .context("run an invalid command with forced colour on a terminal")?;
    expect_that!(output.status.success(), eq(false));
    expect_that!(classify(&output.terminal).has_sgr, eq(false));
    expect_that!(
        String::from_utf8_lossy(&output.terminal),
        contains_substring("Usage")
    );
    Ok(())
}
