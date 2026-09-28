//! Contracts for a build without the `lint` feature.
//!
//! Release binaries build the default feature set, which leaves the manifest
//! linter out until v0.2.0. These tests make that a checked property of the
//! default build rather than an intention: `check` does not parse, `help check`
//! is not a topic, and the top-level help does not list it. They compile only
//! when the feature is off; `check_command_tests` covers the command when it is
//! on.

#![cfg(not(feature = "lint"))]

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use tempfile::TempDir;
use test_support::netsuke::{NetsukeRun, run_netsuke_in};

/// Run `netsuke` with `args` from an empty directory.
fn run_in_empty_directory(args: &[&str]) -> Result<NetsukeRun> {
    let directory = TempDir::new().context("create an empty working directory")?;
    run_netsuke_in(directory.path(), args)
}

/// `check` and `help check` are both rejected as unknown subcommands.
///
/// The subcommand name is matched separately from the message because the
/// localized message wraps it in Unicode isolation marks.
#[rstest]
#[case::command(&["--locale", "en-US", "check"])]
#[case::help_topic(&["--locale", "en-US", "help", "check"])]
fn check_is_an_unknown_subcommand(#[case] args: &[&str]) -> Result<()> {
    let run = run_in_empty_directory(args)?;
    ensure!(
        !run.success,
        "{args:?} should fail without the lint feature"
    );
    ensure!(
        run.stderr.contains("Unknown subcommand") && run.stderr.contains("check"),
        "{args:?} should report `check` as an unknown subcommand: {}",
        run.stderr
    );
    Ok(())
}

/// The top-level help lists every other command but not `check`.
#[test]
fn help_does_not_advertise_check() -> Result<()> {
    let run = run_in_empty_directory(&["--locale", "en-US", "--help"])?;
    ensure!(run.success, "top-level help should succeed: {}", run.stderr);
    let commands: Vec<&str> = run
        .stdout
        .lines()
        .skip_while(|line| !line.starts_with("Commands:"))
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    ensure!(
        commands.contains(&"build") && !commands.contains(&"check"),
        "help should list `build` and omit `check`, got {commands:?}"
    );
    Ok(())
}
