//! Contract tests for every Makefile recipe that overrides `RUSTFLAGS`.
//!
//! Setting `RUSTFLAGS` at all overrides the `[build] rustflags` table in
//! `.cargo/config.toml`, so each such recipe re-states the Polonius flag, and
//! each prepends any value the caller already exported rather than discarding
//! it. `test-nextest`, `doctest`, `lint-clippy`'s Clippy line,
//! `lint-whitaker`, and `typecheck` additionally add `-D warnings`;
//! `kani-full` adds only the Polonius flag, because Kani compiles third-party
//! crates the workspace lint policy does not govern. The binary-build recipe
//! and `lint-clippy`'s rustdoc line preserve the caller's value through a
//! different expansion and add neither `-D warnings` nor anything else.
//!
//! The tests extract each assignment from the Makefile, resolve the Make
//! variables it names, and expand the result in a shell. They assert on the
//! flags that expansion yields rather than on recipe text, so they stay valid
//! when the command a recipe runs changes. A guard test fails if a recipe
//! starts setting `RUSTFLAGS` without joining the covered set.

use super::QUOTED_MANIFEST_FLAG;
use super::makefile::{read_repo_file, target_recipe};
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use rstest::rstest;
use std::{collections::BTreeSet, process::Command};

/// The prefix introducing a quoted `RUSTFLAGS` assignment in a recipe.
const RUSTFLAGS_PREFIX: &str = "RUSTFLAGS=\"";

/// A value a caller might already have exported before invoking `make`.
const CALLER_RUSTFLAGS: &str = "-C target-cpu=native";

const DENY_WARNINGS: &str = "-D warnings";

/// A recipe line that overrides `RUSTFLAGS`, and the contract it must meet.
#[derive(Clone, Copy, Debug)]
struct RustflagsCase {
    /// The Make target owning the recipe.
    target: &'static str,
    /// Substring selecting the recipe line. `lint-clippy` sets `RUSTFLAGS` on
    /// two lines — one for rustdoc, one for Clippy — with different contracts.
    line_marker: &'static str,
    /// Whether the recipe adds `-D warnings`.
    denies_warnings: bool,
    /// Whether the recipe must contribute its separator only alongside an
    /// inherited value, so an unset `RUSTFLAGS` leaves no leading space.
    ///
    /// This is the contract the case asserts, deliberately not read back from
    /// the Makefile: inferring it from the assignment would let a rewrite to a
    /// bare `$RUSTFLAGS ` prefix delete the assertion along with the idiom.
    separator_only_when_set: bool,
}

impl RustflagsCase {
    const fn test_nextest() -> Self {
        Self {
            target: "test-nextest",
            line_marker: "nextest run",
            denies_warnings: true,
            separator_only_when_set: true,
        }
    }

    const fn doctest() -> Self {
        Self {
            target: "doctest",
            line_marker: "--doc",
            denies_warnings: true,
            separator_only_when_set: true,
        }
    }

    const fn binary_build() -> Self {
        Self {
            target: "target/%/$(APP)",
            line_marker: "build",
            denies_warnings: false,
            separator_only_when_set: false,
        }
    }

    const fn lint_clippy_rustdoc() -> Self {
        Self {
            target: "lint-clippy",
            line_marker: "doc --no-deps",
            denies_warnings: false,
            separator_only_when_set: false,
        }
    }

    const fn lint_clippy() -> Self {
        Self {
            target: "lint-clippy",
            line_marker: "clippy",
            denies_warnings: true,
            separator_only_when_set: true,
        }
    }

    /// Clippy's `test_support` pass. The marker must be the manifest flag:
    /// `clippy` alone matches the root line, which `recipe_line` finds first.
    const fn lint_clippy_test_support() -> Self {
        Self {
            target: "lint-clippy",
            line_marker: QUOTED_MANIFEST_FLAG,
            denies_warnings: true,
            separator_only_when_set: true,
        }
    }

    const fn lint_whitaker() -> Self {
        Self {
            target: "lint-whitaker",
            line_marker: "$(WHITAKER)",
            denies_warnings: true,
            separator_only_when_set: true,
        }
    }

    // `test_support` is excluded from the root workspace, so `test-nextest`,
    // `doctest`, and `lint-whitaker` each run a second time against its
    // manifest. Those lines set `RUSTFLAGS` too and hold the same contract as
    // their root counterparts. Their markers must select the scoped line
    // rather than the root one, which `recipe_line` would otherwise find
    // first: the root lines match `nextest run`, `--doc`, and `$(WHITAKER)`
    // as well.
    const fn test_nextest_test_support() -> Self {
        Self {
            target: "test-nextest",
            line_marker: QUOTED_MANIFEST_FLAG,
            denies_warnings: true,
            separator_only_when_set: true,
        }
    }

    const fn doctest_test_support() -> Self {
        Self {
            target: "doctest",
            line_marker: QUOTED_MANIFEST_FLAG,
            denies_warnings: true,
            separator_only_when_set: true,
        }
    }

    const fn lint_whitaker_test_support() -> Self {
        Self {
            target: "lint-whitaker",
            line_marker: "cd test_support",
            denies_warnings: true,
            separator_only_when_set: true,
        }
    }

    const fn typecheck() -> Self {
        Self {
            target: "typecheck",
            line_marker: "check",
            denies_warnings: true,
            separator_only_when_set: true,
        }
    }

    const fn kani_full() -> Self {
        Self {
            target: "kani-full",
            line_marker: "$(KANI)",
            denies_warnings: false,
            separator_only_when_set: true,
        }
    }
}

/// Every `RUSTFLAGS`-setting recipe line under contract.
const RUSTFLAGS_CASES: [RustflagsCase; 12] = [
    RustflagsCase::test_nextest(),
    RustflagsCase::test_nextest_test_support(),
    RustflagsCase::doctest(),
    RustflagsCase::doctest_test_support(),
    RustflagsCase::binary_build(),
    RustflagsCase::lint_clippy_rustdoc(),
    RustflagsCase::lint_clippy(),
    RustflagsCase::lint_clippy_test_support(),
    RustflagsCase::lint_whitaker(),
    RustflagsCase::lint_whitaker_test_support(),
    RustflagsCase::typecheck(),
    RustflagsCase::kani_full(),
];

/// Returns the value of a simple `NAME ?= value` or `NAME = value` variable.
fn make_variable(contents: &str, name: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let rest = line.strip_prefix(name)?;
        let value = rest
            .strip_prefix(" ?= ")
            .or_else(|| rest.strip_prefix(" = "))?;
        Some(value.trim().to_owned())
    })
}

/// Extracts the double-quoted `RUSTFLAGS` assignment from a recipe line.
///
/// `RUSTDOCFLAGS="…"` does not contain `RUSTFLAGS="`, so a line setting both
/// still yields the `RUSTFLAGS` value.
fn rustflags_assignment(line: &str) -> Option<&str> {
    let start = line.find(RUSTFLAGS_PREFIX)? + RUSTFLAGS_PREFIX.len();
    let rest = line.get(start..)?;
    let end = rest.find('"')?;
    rest.get(..end)
}

/// Returns the recipe line `case` selects.
fn recipe_line(makefile: &str, case: RustflagsCase) -> Result<String> {
    let recipe = target_recipe(makefile, case.target)
        .with_context(|| format!("Makefile should declare a {} target", case.target))?;
    recipe
        .lines()
        .find(|line| line.contains(RUSTFLAGS_PREFIX) && line.contains(case.line_marker))
        .map(str::trim)
        .map(ToOwned::to_owned)
        .with_context(|| {
            format!(
                "{} should set RUSTFLAGS on a line matching {:?}",
                case.target, case.line_marker
            )
        })
}

/// Returns `case`'s `RUSTFLAGS` assignment as a shell expression.
///
/// Make variable references are resolved, and Make's `$$` escape is reduced to
/// the single `$` the shell receives.
fn shell_expression(makefile: &str, case: RustflagsCase) -> Result<String> {
    let line = recipe_line(makefile, case)?;
    let assignment = rustflags_assignment(&line).with_context(|| {
        format!(
            "{} should assign a double-quoted RUSTFLAGS value",
            case.target
        )
    })?;
    let polonius = make_variable(makefile, "POLONIUS_FLAGS")
        .context("Makefile should define POLONIUS_FLAGS")?;
    let resolved = assignment.replace("$(POLONIUS_FLAGS)", &polonius);
    ensure!(
        !resolved.contains("$("),
        "{}: RUSTFLAGS assignment {resolved:?} names a Make variable this test cannot resolve",
        case.target
    );
    Ok(resolved.replace("$$", "$"))
}

/// Expands `expression` in a shell, exporting `inherited` as `RUSTFLAGS`.
///
/// Only the assignment is expanded; the command the recipe would run is never
/// executed, so no test here invokes Cargo, Kani, nextest, or Dylint.
#[cfg(unix)]
fn expand(expression: &str, inherited: Option<&str>) -> Result<String> {
    ensure!(
        !expression.contains('"') && !expression.contains('`'),
        "the expansion helper cannot safely embed {expression:?}"
    );
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(format!("printf '%s' \"{expression}\""))
        .env_remove("RUSTFLAGS");
    if let Some(value) = inherited {
        command.env("RUSTFLAGS", value);
    }

    let output = command
        .output()
        .with_context(|| format!("expand {expression:?} with sh"))?;
    ensure!(
        output.status.success(),
        "sh should expand {expression:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).context("expanded RUSTFLAGS should be UTF-8")
}

#[test]
fn unit_extracts_the_rustflags_assignment_from_a_recipe_line() {
    assert_eq!(
        rustflags_assignment(r#"	RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings" $(CARGO) x"#),
        Some(r"$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings")
    );
    // A line setting RUSTDOCFLAGS first still yields the RUSTFLAGS value.
    assert_eq!(
        rustflags_assignment(r#"	RUSTDOCFLAGS="-D warnings" RUSTFLAGS="$${RUSTFLAGS-} -Z" x"#),
        Some(r"$${RUSTFLAGS-} -Z")
    );
    assert_eq!(rustflags_assignment("\tcargo build"), None);
}

#[cfg(unix)]
#[rstest]
#[case(RustflagsCase::test_nextest())]
#[case(RustflagsCase::test_nextest_test_support())]
#[case(RustflagsCase::doctest())]
#[case(RustflagsCase::doctest_test_support())]
#[case(RustflagsCase::binary_build())]
#[case(RustflagsCase::lint_clippy_rustdoc())]
#[case(RustflagsCase::lint_clippy())]
#[case(RustflagsCase::lint_clippy_test_support())]
#[case(RustflagsCase::lint_whitaker())]
#[case(RustflagsCase::lint_whitaker_test_support())]
#[case(RustflagsCase::typecheck())]
#[case(RustflagsCase::kani_full())]
fn behavioural_rustflags_recipes_expand_per_contract(#[case] case: RustflagsCase) -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let polonius = make_variable(&makefile, "POLONIUS_FLAGS")
        .context("Makefile should define POLONIUS_FLAGS")?;
    let expression = shell_expression(&makefile, case)?;

    // With inherited flags: the caller's value survives, the Polonius flag is
    // re-stated, and -D warnings appears exactly when the case demands it.
    let inherited = expand(&expression, Some(CALLER_RUSTFLAGS))?;
    ensure!(
        inherited.contains(CALLER_RUSTFLAGS),
        "{} should preserve the caller's RUSTFLAGS, expanded to {inherited:?}",
        case.target
    );
    ensure!(
        inherited.contains(&polonius),
        "{} should re-state {polonius} (setting RUSTFLAGS overrides .cargo/config.toml), expanded to {inherited:?}",
        case.target
    );
    ensure!(
        inherited.contains(DENY_WARNINGS) == case.denies_warnings,
        "{} should {}deny warnings, expanded to {inherited:?}",
        case.target,
        if case.denies_warnings { "" } else { "not " }
    );

    // Without inherited flags: the re-statement still holds, nothing the
    // caller never set is invented, and no separator is stranded where the
    // idiom promises none.
    let bare = expand(&expression, None)?;
    ensure!(
        bare.contains(&polonius),
        "{} should re-state {polonius} even with no inherited RUSTFLAGS, expanded to {bare:?}",
        case.target
    );
    ensure!(
        !bare.contains(CALLER_RUSTFLAGS),
        "{} should not invent flags the caller never set, expanded to {bare:?}",
        case.target
    );
    // `${VAR:+VAR }` contributes its separator only alongside a value, so an
    // unset RUSTFLAGS must not leave a leading space. Recipes spelling the
    // expansion `${VAR-}` tolerate one, so the case declares which contract
    // applies. This is what separates the idiom from a bare `$RUSTFLAGS `
    // prefix, which preserves the caller's flags but strands a separator.
    if case.separator_only_when_set {
        ensure!(
            !bare.starts_with(' '),
            concat!(
                "{} should not emit a leading separator when RUSTFLAGS is ",
                "unset, expanded to {bare:?}",
            ),
            case.target,
            bare = bare
        );
    }
    Ok(())
}

#[test]
fn behavioural_every_rustflags_recipe_line_is_under_contract() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let declared: BTreeSet<String> = makefile
        .lines()
        .filter(|line| line.starts_with('\t') && line.contains(RUSTFLAGS_PREFIX))
        .map(|line| line.trim().to_owned())
        .collect();
    let covered: BTreeSet<String> = RUSTFLAGS_CASES
        .iter()
        .map(|case| recipe_line(&makefile, *case))
        .collect::<Result<_>>()?;

    let uncovered: Vec<&String> = declared.difference(&covered).collect();
    ensure!(
        uncovered.is_empty(),
        "every recipe setting RUSTFLAGS needs a RustflagsCase; uncovered: {uncovered:#?}"
    );
    ensure!(
        covered.len() == RUSTFLAGS_CASES.len(),
        "each RustflagsCase should select a distinct recipe line, {} cases selected {} lines",
        RUSTFLAGS_CASES.len(),
        covered.len()
    );
    Ok(())
}
