//! Check Clap's help and usage rendering against the startup colour policy.

use super::support::{TerminalWorkspace, display_plan_for_args};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use rstest::rstest;
use test_support::terminal::{Attach, classify};

/// Pin Clap help colour for pipes and terminals.
#[googletest::gtest]
#[rstest]
#[case::never_pipe(&["--color", "never", "--help"], &[], false, false)]
#[case::always_pipe(&["--color", "always", "--help"], &[], false, true)]
#[case::auto_terminal(&["--color", "auto", "--help"], &[], true, true)]
#[case::no_color_terminal(&["--color", "auto", "--help"], &[("NO_COLOR", "1")], true, false)]
#[case::environment_always_pipe(&["--help"], &[("NETSUKE_COLOR", "always")], false, true)]
#[case::cli_overrides_environment(&["--color", "never", "--help"], &[("NETSUKE_COLOR", "always")], false, false)]
fn help_colour_matches_the_startup_policy(
    #[case] args: &[&str],
    #[case] extra_env: &[(&str, &str)],
    #[case] stdout_is_terminal: bool,
    #[case] expected_colour: bool,
) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let plan_args = args
        .iter()
        .copied()
        .filter(|arg| *arg != "--help")
        .collect::<Vec<_>>();
    let plan = display_plan_for_args(
        &plan_args,
        extra_env,
        stdout_is_terminal,
        stdout_is_terminal,
    )?;
    let output = if stdout_is_terminal {
        workspace
            .run_pty(args, extra_env, Attach::StdoutOnly)
            .context("run help on a pseudo-terminal")?
            .terminal
    } else {
        workspace
            .run_piped(args, extra_env)
            .context("run piped help")?
            .stdout
    };

    expect_that!(plan.stderr_colour(), eq(expected_colour));
    expect_that!(classify(&output).has_sgr, eq(expected_colour));
    expect_that!(
        String::from_utf8_lossy(&output),
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
    ensure!(!output.status.success(), "invalid command should fail");
    expect_that!(classify(&output.terminal).has_sgr, eq(false));
    expect_that!(
        String::from_utf8_lossy(&output.terminal),
        contains_substring("Usage")
    );
    Ok(())
}
