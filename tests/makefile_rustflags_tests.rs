//! Behavioural tests for `RUSTFLAGS` composition in Makefile recipes.
//!
//! Several recipes set `RUSTFLAGS` so they can add `-D warnings` and the
//! Polonius flag. Setting `RUSTFLAGS` at all overrides the `[build] rustflags`
//! table in `.cargo/config.toml`, which is why each recipe re-states
//! `-Zpolonius=next`, and why each prepends any value the caller already
//! exported instead of discarding it.
//!
//! Matching the recipe text would only prove the Makefile still spells the
//! expansion the same way. These tests instead run each recipe with a stub
//! standing in for the command it invokes, and assert on the `RUSTFLAGS` that
//! stub actually observes, so a broken expansion fails here rather than in CI.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::{TempDir, tempdir};
use test_support::exec::write_exec_with_content;

/// A `RUSTFLAGS`-setting recipe under test.
#[derive(Copy, Clone, Debug)]
struct RecipeCase {
    /// The Make target to run.
    target: &'static str,
    /// The Make variable naming the command the recipe invokes, so the test
    /// can substitute a stub for it.
    command_variable: &'static str,
    /// Substring selecting the invocation under test. `lint-clippy` drives
    /// `$(CARGO)` twice, so its Clippy line must be picked out by argument.
    /// An empty marker matches a recipe's sole invocation.
    invocation_marker: &'static str,
    /// Whether the recipe adds `-D warnings`. `kani-full` deliberately does
    /// not: Kani compiles third-party crates that the workspace lints do not
    /// govern.
    denies_warnings: bool,
}

impl RecipeCase {
    const fn test_nextest() -> Self {
        Self {
            target: "test-nextest",
            command_variable: "CARGO",
            invocation_marker: "nextest run",
            denies_warnings: true,
        }
    }

    const fn doctest() -> Self {
        Self {
            target: "doctest",
            command_variable: "CARGO",
            invocation_marker: "test --doc",
            denies_warnings: true,
        }
    }

    const fn lint_clippy() -> Self {
        Self {
            target: "lint-clippy",
            command_variable: "CARGO",
            invocation_marker: "clippy",
            denies_warnings: true,
        }
    }

    const fn lint_whitaker() -> Self {
        Self {
            target: "lint-whitaker",
            command_variable: "WHITAKER",
            invocation_marker: "--all",
            denies_warnings: true,
        }
    }

    const fn typecheck() -> Self {
        Self {
            target: "typecheck",
            command_variable: "CARGO",
            invocation_marker: "check --all-targets",
            denies_warnings: true,
        }
    }

    const fn kani_full() -> Self {
        Self {
            target: "kani-full",
            command_variable: "KANI",
            invocation_marker: "",
            denies_warnings: false,
        }
    }
}

/// A stub executable that records the `RUSTFLAGS` each invocation observes.
struct StubCommand {
    _temp: TempDir,
    executable: PathBuf,
    log: PathBuf,
}

impl StubCommand {
    fn new() -> Result<Self> {
        let temp = tempdir().context("create stub command directory")?;
        let log = temp.path().join("invocations.log");
        // Each line is the invocation's arguments and its `RUSTFLAGS`,
        // separated by a tab.
        let script = format!(
            r#"#!/bin/sh
printf '%s\t%s\n' "$*" "${{RUSTFLAGS-}}" >> '{log}'
exit 0
"#,
            log = log.display()
        );
        let executable = write_exec_with_content(temp.path(), "stub-command", &script)
            .context("write stub command")?;

        Ok(Self {
            _temp: temp,
            executable,
            log,
        })
    }

    /// Returns the `RUSTFLAGS` seen by the invocation matching `marker`.
    fn observed_rustflags(&self, marker: &str) -> Result<String> {
        let log = fs::read_to_string(&self.log)
            .with_context(|| format!("read {}", self.log.display()))?;
        log.lines()
            .filter_map(|line| line.split_once('\t'))
            .find(|(arguments, _)| arguments.contains(marker))
            .map(|(_, flags)| flags.to_owned())
            .with_context(|| format!("no stub invocation matched {marker:?}, log was {log:?}"))
    }
}

const CALLER_FLAG: &str = "--cfg=caller_supplied_marker";
const POLONIUS_FLAG: &str = "-Zpolonius=next";
const DENY_WARNINGS: &str = "-D warnings";

/// Runs `case.target`, substituting the stub for the command it invokes.
///
/// `inherited` is exported to `make` as the caller's `RUSTFLAGS`; `None`
/// removes the variable so the unset branch of the expansion is exercised.
fn run_recipe(stub: &StubCommand, case: RecipeCase, inherited: Option<&str>) -> Result<()> {
    let mut command = Command::new("make");
    command
        .arg("--no-print-directory")
        .arg("-f")
        .arg("Makefile")
        .arg(format!(
            "{}={}",
            case.command_variable,
            stub.executable.display()
        ))
        .arg(case.target)
        .env_remove("RUSTFLAGS");
    if let Some(value) = inherited {
        command.env("RUSTFLAGS", value);
    }

    let output = command
        .output()
        .with_context(|| format!("run make {}", case.target))?;
    ensure!(
        output.status.success(),
        "make {} should succeed: {}",
        case.target,
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[rstest]
#[case(RecipeCase::test_nextest())]
#[case(RecipeCase::doctest())]
#[case(RecipeCase::lint_clippy())]
#[case(RecipeCase::lint_whitaker())]
#[case(RecipeCase::typecheck())]
#[case(RecipeCase::kani_full())]
fn behavioural_recipes_prepend_inherited_rustflags(#[case] case: RecipeCase) -> Result<()> {
    let stub = StubCommand::new()?;
    run_recipe(&stub, case, Some(CALLER_FLAG))?;
    let observed = stub.observed_rustflags(case.invocation_marker)?;

    ensure!(
        observed.starts_with(&format!("{CALLER_FLAG} ")),
        "make {} should prepend the caller's RUSTFLAGS, observed {observed:?}",
        case.target
    );
    ensure!(
        observed.contains(POLONIUS_FLAG),
        "make {} should re-state {POLONIUS_FLAG} because setting RUSTFLAGS \
         overrides .cargo/config.toml, observed {observed:?}",
        case.target
    );
    ensure!(
        observed.contains(DENY_WARNINGS) == case.denies_warnings,
        "make {} should {} deny warnings, observed {observed:?}",
        case.target,
        if case.denies_warnings { "" } else { "not" }
    );
    Ok(())
}

#[rstest]
#[case(RecipeCase::test_nextest())]
#[case(RecipeCase::doctest())]
#[case(RecipeCase::lint_clippy())]
#[case(RecipeCase::lint_whitaker())]
#[case(RecipeCase::typecheck())]
#[case(RecipeCase::kani_full())]
fn behavioural_recipes_add_no_separator_without_inherited_rustflags(
    #[case] case: RecipeCase,
) -> Result<()> {
    let stub = StubCommand::new()?;
    run_recipe(&stub, case, None)?;
    let observed = stub.observed_rustflags(case.invocation_marker)?;

    // The `:+` expansion contributes the separator only alongside a value, so
    // an unset `RUSTFLAGS` must not leave a leading space.
    ensure!(
        !observed.starts_with(' '),
        "make {} should not emit a leading separator when RUSTFLAGS is unset, \
         observed {observed:?}",
        case.target
    );
    let expected_first = if case.denies_warnings {
        DENY_WARNINGS
    } else {
        POLONIUS_FLAG
    };
    ensure!(
        observed.starts_with(expected_first),
        "make {} should start with {expected_first:?} when RUSTFLAGS is unset, \
         observed {observed:?}",
        case.target
    );
    ensure!(
        observed.contains(POLONIUS_FLAG),
        "make {} should re-state {POLONIUS_FLAG}, observed {observed:?}",
        case.target
    );
    Ok(())
}
