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

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8Path;
use rstest::rstest;

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

/// Yield the arguments Whitaker reads as its own.
///
/// Everything from the standalone `--` token onward is forwarded to Cargo, so
/// `--all-targets` and `--all-features` live there and select nothing.
/// Splitting on whitespace keeps the check from either missing a real `--all`
/// or matching a Cargo flag that merely starts with it, and it treats a tab, a
/// doubled space, and a line that ends at the separator alike — none of which
/// a search for the substring `" -- "` would find.
fn library_selection_arguments(line: &str) -> impl Iterator<Item = &str> + '_ {
    line.split_whitespace().take_while(|token| *token != "--")
}

/// Return whatever follows `WHITAKER_SUITE_SELECTION` on an assigning line.
///
/// The name is matched as a whole token so the text after it is the operator
/// rather than the tail of a longer name: a prefix match would read
/// `WHITAKER_SUITE_SELECTION_EXTRA = --all` as a definition of this variable.
/// The name is looked for anywhere on the line rather than only at its start,
/// because GNU Make accepts it under an `export`, `override` or `private`
/// prefix and in a target-specific assignment, and each of those assigns the
/// variable as surely as the bare form. A commented-out line assigns nothing,
/// so it is skipped rather than counted as a definition.
fn suite_selection_assignment(line: &str) -> Option<&str> {
    const NAME: &str = "WHITAKER_SUITE_SELECTION";

    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return None;
    }
    let start = trimmed.match_indices(NAME).find_map(|(index, _)| {
        // `get` rather than a range index: the offsets come from `match_indices`,
        // so they are character boundaries, but a plain index would still be a
        // panic site if that ever stopped being true.
        let before = trimmed.get(..index)?.chars().next_back();
        let after = trimmed.get(index + NAME.len()..)?.chars().next();
        let is_boundary =
            |character: Option<char>| character.is_none_or(|value| !is_identifier(value));
        (is_boundary(before) && is_boundary(after)).then_some(index)
    })?;
    trimmed.get(start + NAME.len()..)
}

/// Report whether `character` can continue a Makefile variable name.
fn is_identifier(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// Return the operator of the Makefile's `WHITAKER_SUITE_SELECTION` assignment.
///
/// `+=` is reported as its own operator rather than ignored: it assigns, so a
/// second such line is a second definition and has to be caught as one.
fn selection_operator(line: &str) -> Option<char> {
    let rest = suite_selection_assignment(line)?.trim_start();
    let mut characters = rest.chars();
    match characters.next()? {
        '+' if characters.next() == Some('=') => Some('+'),
        operator @ ('?' | ':' | '=') => Some(operator),
        _ => None,
    }
}

/// Return the Makefile's single `WHITAKER_SUITE_SELECTION` definition.
///
/// Checking one matching line is not enough. A later assignment — `=` again, or
/// `+=`, which appends — can add the standalone libraries to a selection whose
/// first line is correct, and the line this module inspects would still read as
/// expected. The definition is therefore required to occur exactly once, and a
/// duplicate is reported with every line that matched.
///
/// A command-line assignment, `make WHITAKER_SUITE_SELECTION=… lint-whitaker`,
/// still wins over every form accepted here. That is deliberate and not a hole
/// this check should close: it is an explicit instruction for one invocation,
/// in the same way the suite substitutes `KANI=`, `UV=` and `PROVER_TOOLS=` to
/// exercise a branch, whereas an inherited environment value or a second in-file
/// definition arrives without anyone asking for it at the call site.
///
/// # Errors
///
/// Returns an error if the name is defined more than once, or not at all.
fn suite_selection_definition(makefile: &str) -> Result<&str> {
    let definitions: Vec<&str> = makefile
        .lines()
        .filter(|line| selection_operator(line).is_some())
        .collect();
    let [selection] = definitions.as_slice() else {
        bail!(
            concat!(
                "the Makefile should define WHITAKER_SUITE_SELECTION exactly ",
                "once, since a second assignment can append `--all` to the ",
                "pinned selection; found {count}: {definitions:?}",
            ),
            count = definitions.len(),
            definitions = format!("{definitions:?}")
        );
    };
    Ok(*selection)
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
        ensure!(
            !library_selection_arguments(line).any(|token| token == "--all"),
            concat!(
                "`--all` loads the standalone per-lint libraries as well as the ",
                "suite, which registers several lints twice; found {line:?}",
            ),
            line = line
        );
    }

    let selection = suite_selection_definition(&makefile)?;
    ensure!(
        matches!(selection_operator(selection), Some('=' | ':')),
        concat!(
            "WHITAKER_SUITE_SELECTION must be assigned unconditionally: `?=` ",
            "defers to an exported value and `+=` appends to one, so either ",
            "lets an outside value add the standalone libraries the suite ",
            "already registers; found {selection:?}",
        ),
        selection = selection
    );
    for fragment in ["--lib whitaker_suite", "--fail-on-no-libraries"] {
        ensure!(
            selection.contains(fragment),
            "WHITAKER_SUITE_SELECTION should carry {fragment}, found {selection:?}"
        );
    }
    Ok(())
}

/// Pin the operator read from a line that names the variable.
///
/// The name occurs on recipe lines as well as in definitions, so the helper
/// has to return no operator there. `--all $(WHITAKER_SUITE_SELECTION)
/// --no-deps` is the spelling that matters: it names the suite *and* asks for
/// the standalone libraries, so reading it as a definition would let the
/// duplicate check pass on the one line it exists to catch.
#[rstest]
#[case::assign("WHITAKER_SUITE_SELECTION = --lib whitaker_suite", Some('='))]
#[case::conditional("WHITAKER_SUITE_SELECTION ?= --lib whitaker_suite", Some('?'))]
#[case::immediate("WHITAKER_SUITE_SELECTION := --lib whitaker_suite", Some(':'))]
#[case::append("export WHITAKER_SUITE_SELECTION += --all", Some('+'))]
#[case::target_specific(
    "lint-whitaker: WHITAKER_SUITE_SELECTION = --lib whitaker_suite",
    Some('=')
)]
#[case::commented_out("# WHITAKER_SUITE_SELECTION = --all", None)]
#[case::referenced_in_a_recipe("$(WHITAKER) --all $(WHITAKER_SUITE_SELECTION) --no-deps", None)]
#[case::subscripted_name("WHITAKER_SUITE_SELECTION_EXTRA = --all", None)]
fn only_an_assignment_yields_a_selection_operator(
    #[case] line: &str,
    #[case] expected: Option<char>,
) {
    assert_eq!(
        selection_operator(line),
        expected,
        "the operator read from {line:?} should be {expected:?}"
    );
}
