//! Contracts pinning how `make lint-whitaker` invokes Whitaker.
//!
//! The lint gate is configuration, so a wrong invocation is silent: it either
//! lints less than intended or refuses to start at all. Two properties matter.
//!
//! First, the gate must run twice — once at the repository root for
//! `netsuke-build` and once from `test_support/` — because the two packages
//! carry different `dylint.toml` policies. A single invocation would apply the
//! root policy to `test_support` or leave the crate unlinted.
//!
//! Second, both invocations must select `whitaker_suite` by name and refuse to
//! run without it. The rolling release publishes a standalone library for every
//! lint as well as the suite, and the suite already registers all of them, so
//! `--all` loads several lints twice and rustc aborts with `duplicate
//! specification of lint`. `--fail-on-no-libraries` is the other half: without
//! it a missing suite leaves Whitaker running a lint-free `cargo check` that
//! reports success.
//!
//! These are deterministic file checks; they never invoke Whitaker itself.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;

use crate::makefile::{read_repo_file, target_recipe};

/// Collect the package names selected by a Makefile recipe line.
///
/// Substring matching cannot tell `--package test_support` apart from
/// `--package test_support-extra`, nor spot a second selection appended later,
/// so the line is tokenized and whole arguments are compared. Both the
/// separated and `--package=value` spellings are recognized, as is the `-p`
/// short form.
fn package_selections(line: &str) -> Vec<&str> {
    let mut tokens = line.split_whitespace();
    let mut packages = Vec::new();
    while let Some(token) = tokens.next() {
        if let Some(value) = token.strip_prefix("--package=") {
            packages.push(value);
        } else if token == "--package" || token == "-p" {
            packages.extend(tokens.next());
        }
    }
    packages
}

/// Return the part of a recipe line that Whitaker reads as its own arguments.
///
/// Everything after the `--` separator is forwarded to Cargo, so
/// `--all-targets` and `--all-features` live there and select nothing.
/// Returning the prefix keeps the library-selection check from either missing
/// a real `--all` or matching a Cargo flag that merely starts with it.
fn library_selection_arguments(line: &str) -> &str {
    line.split_once(" -- ")
        .map_or(line, |(selection, _)| selection)
}

#[test]
fn lint_whitaker_also_runs_inside_test_support() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let recipe = target_recipe(&makefile, "lint-whitaker")
        .context("the Makefile should declare a lint-whitaker target")?;

    let invocations: Vec<&str> = recipe
        .lines()
        .filter(|line| line.contains("$(WHITAKER)"))
        .collect();
    ensure!(
        invocations.len() == 2,
        concat!(
            "lint-whitaker should invoke Whitaker twice — once at the ",
            "repository root for netsuke and once inside test_support — ",
            "found {count}: {recipe:?}",
        ),
        count = invocations.len(),
        recipe = recipe
    );

    // Counted separately rather than with `any`, which two test_support runs
    // would satisfy while leaving the whole application crate unlinted.
    let (scoped, root): (Vec<&str>, Vec<&str>) = invocations
        .iter()
        .partition(|line| line.contains("cd test_support"));
    ensure!(
        scoped.len() == 1,
        concat!(
            "exactly one lint-whitaker invocation should run from ",
            "test_support/, found {count}: {recipe:?}",
        ),
        count = scoped.len(),
        recipe = recipe
    );
    ensure!(
        scoped.first().is_some_and(|line| {
            line.contains(r#"DYLINT_TOML="$$(cat dylint.toml)""#)
                && package_selections(line) == ["test_support"]
                && line.contains("--no-deps")
        }),
        concat!(
            "the scoped Whitaker invocation must explicitly load ",
            "test_support/dylint.toml and select exactly test_support after ",
            "workspace membership changes Cargo's configuration and package ",
            "roots; found {scoped:?}",
        ),
        scoped = format!("{scoped:?}")
    );
    ensure!(
        root.len() == 1,
        concat!(
            "exactly one lint-whitaker invocation should run from the ",
            "repository root, found {count}: {recipe:?}",
        ),
        count = root.len(),
        recipe = recipe
    );
    ensure!(
        root.first().is_some_and(|line| {
            package_selections(line) == ["netsuke-build"] && line.contains("--no-deps")
        }),
        concat!(
            "the root Whitaker invocation must select exactly netsuke-build and skip ",
            "dependency checks so test_support loads its own dylint.toml; ",
            "found {root:?}",
        ),
        root = format!("{root:?}")
    );
    Ok(())
}

#[test]
fn lint_whitaker_selects_the_suite_and_requires_it() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let recipe = target_recipe(&makefile, "lint-whitaker")
        .context("the Makefile should declare a lint-whitaker target")?;

    let invocations: Vec<&str> = recipe
        .lines()
        .filter(|line| line.contains("$(WHITAKER)"))
        .collect();
    ensure!(
        !invocations.is_empty(),
        "lint-whitaker should invoke Whitaker, found {recipe:?}"
    );
    for line in &invocations {
        ensure!(
            line.contains("$(WHITAKER_SUITE_SELECTION)"),
            concat!(
                "every Whitaker invocation must select the suite through ",
                "WHITAKER_SUITE_SELECTION; found {line:?}",
            ),
            line = line
        );
        // An exact token, not a substring: `--all $(WHITAKER_SUITE_SELECTION)
        // --no-deps` is the dangerous spelling, and the old `--all --no-deps`
        // pair would not have caught it.
        let selection = library_selection_arguments(line);
        ensure!(
            !selection.split_whitespace().any(|token| token == "--all"),
            concat!(
                "`--all` loads the standalone per-lint libraries as well as the ",
                "suite, which registers several lints twice; found {line:?}",
            ),
            line = line
        );
    }

    let selection = makefile
        .lines()
        .find(|line| line.starts_with("WHITAKER_SUITE_SELECTION"))
        .context("the Makefile should define WHITAKER_SUITE_SELECTION")?;
    for fragment in ["--lib whitaker_suite", "--fail-on-no-libraries"] {
        ensure!(
            selection.contains(fragment),
            "WHITAKER_SUITE_SELECTION should carry {fragment}, found {selection:?}"
        );
    }
    Ok(())
}
