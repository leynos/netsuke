//! Behavioural boundary tests for the Markdown halves of `make fmt` and
//! `make check-fmt`.
//!
//! `tests/workflow_contracts/markdown_gates_test.py` is the static contract:
//! it reads the Makefile text and holds both recipes to the shared
//! `MDTABLEFIX_SELECT` and `MDTABLEFIX_RULES` variables. This module runs the
//! recipes. Every external program a recipe reaches — Cargo, Ruff,
//! `mdtablefix`, and `markdownlint-cli2` — is a fake executable that appends
//! its name and arguments to one invocation log and exits with a code the
//! test chooses, supplied through the Makefile's own `CARGO`, `RUFF`,
//! `MDTABLEFIX`, and `MDLINT` overrides. No host installation is consulted.
//!
//! The recipes run inside a throwaway Git repository holding tracked,
//! untracked, and ignored Markdown, and once inside a repository with no
//! Markdown at all. That is deliberate: file selection belongs to
//! `mdtablefix --git` (0.6.0 defines an empty selection as a success that
//! prints nothing and reads no standard input), so the tests assert that Make
//! hands over the selection flags and nothing else — no paths, no standard
//! input, and no shell guard for the empty case.

use super::markdown_recipe_harness::{
    ExitCodes, Harness, Invocation, RULES, RULES_ASSIGNMENT, SELECTION, empty_selection_workspace,
    populated_workspace,
};
use super::read_repo_file;
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use rstest::rstest;
use std::path::Path;

/// The exact `mdtablefix` argument list a recipe must pass for `mode`.
fn expected_mdtablefix_arguments(mode: &str) -> Vec<String> {
    std::iter::once(mode)
        .chain(SELECTION)
        .chain(RULES)
        .map(ToOwned::to_owned)
        .collect()
}

/// Return the single `mdtablefix` call, failing on any other count.
fn the_mdtablefix_call(invocations: &[Invocation]) -> Result<&Invocation> {
    let calls: Vec<&Invocation> = invocations
        .iter()
        .filter(|invocation| invocation.program == "mdtablefix")
        .collect();
    match calls.as_slice() {
        [only] => Ok(only),
        _ => anyhow::bail!("the recipe should call mdtablefix exactly once, got {invocations:#?}"),
    }
}

fn program_sequence(invocations: &[Invocation]) -> Vec<&str> {
    invocations
        .iter()
        .map(|invocation| invocation.program.as_str())
        .collect()
}

#[test]
fn behavioural_shared_rule_set_matches_the_makefile_assignment() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let assignment = makefile
        .lines()
        .find_map(|line| line.strip_prefix(RULES_ASSIGNMENT))
        .context("the Makefile should assign MDTABLEFIX_RULES")?;
    let declared: Vec<&str> = assignment.split_whitespace().collect();
    ensure!(
        declared == RULES,
        "this module's RULES must equal the Makefile's MDTABLEFIX_RULES, found {declared:?}"
    );
    Ok(())
}

#[test]
fn behavioural_fmt_formats_markdown_through_mdtablefix_then_markdownlint() -> Result<()> {
    let harness = Harness::new(populated_workspace()?)?;
    let before = harness.markdown_bytes()?;

    let output = harness.run("fmt", ExitCodes::default())?;

    ensure!(
        output.status.success(),
        "make fmt should succeed when every tool succeeds: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let invocations = harness.invocations()?;
    ensure!(
        program_sequence(&invocations)
            == ["cargo", "ruff", "ruff", "mdtablefix", "markdownlint-cli2"],
        "make fmt must preserve the recipe order, got {invocations:#?}"
    );
    let first = invocations
        .first()
        .context("make fmt should record a call")?;
    ensure!(
        first.arguments == ["fmt", "--all"],
        "make fmt should format Rust first, got {first:?}"
    );
    let mdtablefix = the_mdtablefix_call(&invocations)?;
    ensure!(
        mdtablefix.arguments == expected_mdtablefix_arguments("--in-place"),
        "make fmt must pass --in-place, the Git selection, and the shared rules, got {mdtablefix:?}"
    );
    let markdownlint = invocations
        .last()
        .context("the last invocation should exist")?;
    ensure!(
        markdownlint.program == "markdownlint-cli2"
            && markdownlint.arguments.first().map(String::as_str) == Some("--fix"),
        "make fmt must run markdownlint-cli2 --fix after mdtablefix, got {markdownlint:?}"
    );
    ensure!(
        harness.markdown_bytes()? == before,
        "the fakes never touch the workspace, so the Markdown must be unchanged"
    );
    Ok(())
}

#[test]
fn behavioural_check_fmt_asks_mdtablefix_for_drift_over_the_git_selection() -> Result<()> {
    let harness = Harness::new(populated_workspace()?)?;

    let output = harness.run("check-fmt", ExitCodes::default())?;

    ensure!(
        output.status.success(),
        "make check-fmt should succeed on a clean tree: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let invocations = harness.invocations()?;
    ensure!(
        program_sequence(&invocations) == ["cargo", "ruff", "mdtablefix"],
        "make check-fmt must preserve the recipe order, got {invocations:#?}"
    );
    let first = invocations
        .first()
        .context("make check-fmt should record a call")?;
    ensure!(
        first.arguments == ["fmt", "--all", "--", "--check"],
        "make check-fmt should check Rust formatting read-only, got {first:?}"
    );
    let mdtablefix = the_mdtablefix_call(&invocations)?;
    ensure!(
        mdtablefix.arguments == expected_mdtablefix_arguments("--check"),
        "make check-fmt must pass --check, the Git selection, and the shared rules, got {mdtablefix:?}"
    );
    ensure!(
        !mdtablefix
            .arguments
            .iter()
            .any(|argument| argument == "--in-place"),
        "make check-fmt must never rewrite files, got {mdtablefix:?}"
    );
    ensure!(
        !mdtablefix
            .arguments
            .iter()
            .any(|argument| Path::new(argument)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))),
        "selection belongs to mdtablefix --git; Make must pass no paths, got {mdtablefix:?}"
    );
    Ok(())
}

#[test]
fn behavioural_check_fmt_succeeds_when_git_selects_no_markdown() -> Result<()> {
    let harness = Harness::new(empty_selection_workspace()?)?;

    let output = harness.run("check-fmt", ExitCodes::default())?;

    ensure!(
        output.status.success(),
        "an empty Git selection is mdtablefix's success case, not Make's: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let invocations = harness.invocations()?;
    let mdtablefix = the_mdtablefix_call(&invocations)?;
    ensure!(
        mdtablefix.arguments == expected_mdtablefix_arguments("--check"),
        "the empty case must reach mdtablefix with the same flags, got {mdtablefix:?}"
    );
    ensure!(
        mdtablefix.stdin_bytes == Some(0),
        "Make must not feed mdtablefix anything on standard input, got {mdtablefix:?}"
    );
    Ok(())
}

#[rstest]
#[case::drift(1)]
#[case::operational_failure(2)]
fn behavioural_check_fmt_fails_when_mdtablefix_does(#[case] exit_code: u8) -> Result<()> {
    let harness = Harness::new(populated_workspace()?)?;

    let output = harness.run(
        "check-fmt",
        ExitCodes {
            mdtablefix: exit_code,
            ..ExitCodes::default()
        },
    )?;

    ensure!(
        !output.status.success(),
        "make check-fmt must fail when mdtablefix exits {exit_code}"
    );
    let invocations = harness.invocations()?;
    the_mdtablefix_call(&invocations)?;
    Ok(())
}

#[rstest]
#[case::cargo(ExitCodes { cargo: 1, ..ExitCodes::default() }, ["cargo"].as_slice())]
#[case::ruff(ExitCodes { ruff: 1, ..ExitCodes::default() }, ["cargo", "ruff"].as_slice())]
fn behavioural_check_fmt_stops_at_the_first_failing_formatter(
    #[case] exits: ExitCodes,
    #[case] expected_sequence: &[&str],
) -> Result<()> {
    let harness = Harness::new(populated_workspace()?)?;

    let output = harness.run("check-fmt", exits)?;

    ensure!(
        !output.status.success(),
        "make check-fmt must propagate an earlier formatter failure"
    );
    let invocations = harness.invocations()?;
    ensure!(
        program_sequence(&invocations) == expected_sequence,
        "a failing formatter must stop the recipe before mdtablefix, got {invocations:#?}"
    );
    Ok(())
}
