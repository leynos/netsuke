//! Contract model for the Makefile variables that assign `RUSTFLAGS`.
//!
//! No recipe spells the value out any more: each composes one of a small set of
//! Make variables, so the variables are what this module contracts. That
//! indirection is exactly the hazard a text-walking contract can be blind to, so
//! the completeness test has two halves — every `RUSTFLAGS="` in the file must
//! be one of the contracted variables, and every recipe that sets `RUSTFLAGS`
//! must do so through one of them. Neither half alone would notice a recipe that
//! quietly went back to composing its own.
//!
//! What each variable then *expands* to is checked in the [`expansion`]
//! submodule, which asks Make and then a shell. This module holds the model both
//! halves share: the table of variables with the policy each row carries, and
//! the reading that pulls an assignment out of a line.
//!
//! The parent `makefile_test_target` module supplies the repository-file and
//! recipe-lookup helpers.

use super::read_repo_file;
// The recipe lookup is Unix-only, so the import is too: an unconditional one is
// an unused-import error on Windows.
#[cfg(unix)]
use super::target_recipe;
use anyhow::{Result, ensure};
// Every `context` call sits in the Unix-only gate-recipe test, so the trait
// import is gated with it.
#[cfg(unix)]
use anyhow::Context;
use camino::Utf8Path;
use std::collections::BTreeSet;

/// The prefix introducing a quoted `RUSTFLAGS` assignment.
const RUSTFLAGS_PREFIX: &str = "RUSTFLAGS=\"";

/// A Make variable holding a `RUSTFLAGS` assignment, and the policy it carries.
#[derive(Clone, Copy, Debug)]
struct RustflagsVariable {
    /// The Make variable's name.
    name: &'static str,
    /// Whether the composed value denies warnings. The gate targets do; a
    /// plain build must not, or `make build` silently becomes a gate.
    ///
    /// Read only by the shell-expansion test in the `expansion` submodule, which
    /// needs a real shell and so runs on Unix alone. The field stays on every
    /// platform because the table below is one list, not one per platform.
    #[cfg_attr(
        not(unix),
        expect(dead_code, reason = "read only by the Unix-only expansion test")
    )]
    denies_warnings: bool,
    /// Whether the composed value carries the build standard's flags. The
    /// release exclusion turns on this being false.
    ///
    /// Read only by the shell-expansion test, as above.
    #[cfg_attr(
        not(unix),
        expect(dead_code, reason = "read only by the Unix-only expansion test")
    )]
    carries_standard: bool,
}

/// Every Make variable that assigns `RUSTFLAGS`.
///
/// The four are distinguished by two independent policies, and the tests below
/// assert each policy rather than the variable's name, so a variable whose
/// definition drifts from its row fails even when the name is unchanged.
const RUSTFLAGS_VARIABLES: [RustflagsVariable; 4] = [
    RustflagsVariable {
        name: "GATE_RUSTFLAGS",
        denies_warnings: true,
        carries_standard: true,
    },
    RustflagsVariable {
        name: "DEBUG_RUSTFLAGS",
        denies_warnings: false,
        carries_standard: true,
    },
    RustflagsVariable {
        name: "RELEASE_RUSTFLAGS",
        denies_warnings: false,
        carries_standard: false,
    },
    // Kani denies warnings but takes none of the standard: it compiles through
    // `kani-compiler` on its own bundled toolchain, where neither flag applies.
    RustflagsVariable {
        name: "KANI_RUSTFLAGS",
        denies_warnings: true,
        carries_standard: false,
    },
];

/// Extracts the double-quoted `RUSTFLAGS` assignment from a line.
///
/// `RUSTDOCFLAGS="…"` does not contain `RUSTFLAGS="`, so a line setting both
/// still yields the `RUSTFLAGS` value.
fn rustflags_assignment(line: &str) -> Option<&str> {
    let start = line.find(RUSTFLAGS_PREFIX)? + RUSTFLAGS_PREFIX.len();
    let rest = line.get(start..)?;
    let end = rest.rfind('"')?;
    rest.get(..end)
}

#[test]
fn unit_extracts_the_rustflags_assignment_from_a_line() {
    assert_eq!(
        rustflags_assignment(r#"RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-D warnings""#),
        Some(r"${RUSTFLAGS:+$RUSTFLAGS }-D warnings")
    );
    // A line setting RUSTDOCFLAGS first still yields the RUSTFLAGS value.
    assert_eq!(
        rustflags_assignment(r#"RUSTDOCFLAGS="-D warnings" RUSTFLAGS="${RUSTFLAGS-} -Z""#),
        Some(r"${RUSTFLAGS-} -Z")
    );
    assert_eq!(rustflags_assignment("cargo build"), None);
}

#[test]
fn behavioural_every_rustflags_assignment_is_a_contracted_variable() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let declared: BTreeSet<String> = makefile
        .lines()
        .filter(|line| line.contains(RUSTFLAGS_PREFIX))
        .map(|line| line.trim().to_owned())
        .collect();
    let names: Vec<&str> = RUSTFLAGS_VARIABLES
        .iter()
        .map(|variable| variable.name)
        .collect();

    for line in &declared {
        ensure!(
            names.iter().any(|name| line.starts_with(name)),
            "every RUSTFLAGS assignment must be one of {names:?}; found {line:?}"
        );
    }
    ensure!(
        declared.len() == RUSTFLAGS_VARIABLES.len(),
        "expected one assignment per contracted variable, found {declared:#?}"
    );
    Ok(())
}

#[test]
fn behavioural_every_recipe_sets_rustflags_through_a_contracted_variable() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    // The two halves catch different edits. The assignment test above would
    // still pass if a recipe stopped referencing any variable and simply
    // dropped the flags; this one would still pass if a variable's definition
    // were rewritten. Only together do they hold the composition.
    let offenders: Vec<&str> = makefile
        .lines()
        .filter(|line| line.starts_with('\t') && line.contains("RUSTFLAGS"))
        .filter(|line| {
            !RUSTFLAGS_VARIABLES
                .iter()
                .any(|variable| line.contains(&format!("$({})", variable.name)))
        })
        .collect();
    ensure!(
        offenders.is_empty(),
        "every recipe setting RUSTFLAGS must compose a contracted variable; found {offenders:#?}"
    );

    for variable in RUSTFLAGS_VARIABLES {
        let reference = format!("$({})", variable.name);
        ensure!(
            makefile
                .lines()
                .any(|line| line.starts_with('\t') && line.contains(&reference)),
            "{} is defined but no recipe composes it",
            variable.name
        );
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn behavioural_the_gate_variable_reaches_every_gate_recipe() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    for target in [
        "test-nextest",
        "doctest",
        "lint-clippy",
        "lint-whitaker",
        "typecheck",
    ] {
        let recipe = target_recipe(&makefile, target)
            .with_context(|| format!("Makefile should declare a {target} target"))?;
        ensure!(
            recipe.contains("$(GATE_RUSTFLAGS)"),
            "{target} should compose GATE_RUSTFLAGS, found {recipe:?}"
        );
    }
    Ok(())
}

// Declared last, as in `test_support/src/fs.rs`: the module's own model reads
// first, and the submodule carries the machinery that needs a real shell.
#[cfg(unix)]
#[path = "rustflags_expansion.rs"]
mod expansion;
