//! Contract model for Makefile recipes that set `RUSTFLAGS`.
//!
//! Each recipe line that assigns `RUSTFLAGS` is described by a
//! [`RustflagsCase`] naming its target and the substring selecting the line.
//! The module extracts the double-quoted assignment from the recipe and (on
//! Unix) expands it in a real shell — without running the recipe's command —
//! so the tests assert what Cargo would actually receive: inherited caller
//! flags preserved, and `-D warnings` applied. Every recipe that sets
//! `RUSTFLAGS` at all does so to deny warnings while conditionally preserving
//! an inherited value, so both contracts are asserted unconditionally; a
//! recipe needing a different policy would fail these assertions rather than
//! slip through. A completeness test walks the Makefile and fails when any
//! `RUSTFLAGS`-setting line lacks a case, so new recipes join the contract or
//! break the build. The parent `makefile_test_target` module supplies the
//! repository-file and recipe-lookup helpers.

use super::{read_repo_file, target_recipe};
use anyhow::{Context, Result, ensure};
#[cfg(unix)]
use assert_cmd::Command;
use camino::Utf8Path;
use std::collections::BTreeSet;

/// The prefix introducing a quoted `RUSTFLAGS` assignment in a recipe.
const RUSTFLAGS_PREFIX: &str = "RUSTFLAGS=\"";

/// A value a caller might already have exported before invoking `make`.
#[cfg(unix)]
const CALLER_RUSTFLAGS: &str = "-C target-cpu=native";

#[cfg(unix)]
const DENY_WARNINGS: &str = "-D warnings";
/// A recipe line that overrides `RUSTFLAGS`, and the contract it must meet.
#[derive(Clone, Copy, Debug)]
struct RustflagsCase {
    /// The Make target owning the recipe.
    target: &'static str,
    /// Substring selecting the recipe line.
    line_marker: &'static str,
}

impl RustflagsCase {
    const fn test_nextest() -> Self {
        Self {
            target: "test-nextest",
            line_marker: "nextest run",
        }
    }

    const fn doctest() -> Self {
        Self {
            target: "doctest",
            line_marker: "--doc",
        }
    }

    const fn lint_rustdoc() -> Self {
        Self {
            target: "lint-clippy",
            line_marker: "doc --workspace",
        }
    }

    const fn lint_clippy() -> Self {
        Self {
            target: "lint-clippy",
            line_marker: "clippy",
        }
    }

    const fn lint_whitaker() -> Self {
        Self {
            target: "lint-whitaker",
            line_marker: "$(WHITAKER)",
        }
    }

    const fn lint_whitaker_test_support() -> Self {
        Self {
            target: "lint-whitaker",
            line_marker: "cd test_support",
        }
    }

    const fn typecheck() -> Self {
        Self {
            target: "typecheck",
            line_marker: "check",
        }
    }
}

/// Every `RUSTFLAGS`-setting recipe line under contract.
const RUSTFLAGS_CASES: [RustflagsCase; 7] = [
    RustflagsCase::test_nextest(),
    RustflagsCase::doctest(),
    RustflagsCase::lint_rustdoc(),
    RustflagsCase::lint_clippy(),
    RustflagsCase::lint_whitaker(),
    RustflagsCase::lint_whitaker_test_support(),
    RustflagsCase::typecheck(),
];
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
/// Make's `$$` escape is reduced to the single `$` the shell receives. No
/// assignment may name a Make variable, since this test cannot resolve one.
fn shell_expression(makefile: &str, case: RustflagsCase) -> Result<String> {
    let line = recipe_line(makefile, case)?;
    let assignment = rustflags_assignment(&line).with_context(|| {
        format!(
            "{} should assign a double-quoted RUSTFLAGS value",
            case.target
        )
    })?;
    let resolved = assignment.replace("$$", "$");
    ensure!(
        !resolved.contains("$("),
        "{}: RUSTFLAGS assignment {resolved:?} names a Make variable this test cannot resolve",
        case.target
    );
    Ok(resolved)
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

#[test]
fn unit_rejects_escaped_shell_command_substitution() {
    let makefile = concat!("unsafe-recipe:\n", "\tRUSTFLAGS=\"$$(date)\" echo unsafe\n",);
    let case = RustflagsCase {
        target: "unsafe-recipe",
        line_marker: "echo unsafe",
    };

    assert!(
        shell_expression(makefile, case).is_err(),
        "escaped shell command substitution should be rejected"
    );
}

#[cfg(unix)]
#[test]
fn behavioural_rustflags_recipes_preserve_inherited_flags() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    for case in RUSTFLAGS_CASES {
        let expanded = expand(&shell_expression(&makefile, case)?, Some(CALLER_RUSTFLAGS))?;

        ensure!(
            expanded.contains(CALLER_RUSTFLAGS),
            "{} inherited-RUSTFLAGS contract should hold, expanded to {expanded:?}",
            case.target
        );
        ensure!(
            expanded.contains(DENY_WARNINGS),
            "{} should deny warnings, expanded to {expanded:?}",
            case.target
        );
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn behavioural_rustflags_recipes_are_well_formed_without_inherited_flags() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    for case in RUSTFLAGS_CASES {
        let expression = shell_expression(&makefile, case)?;
        let expanded = expand(&expression, None)?;

        ensure!(
            !expanded.contains(CALLER_RUSTFLAGS),
            "{} should not invent flags the caller never set, expanded to {expanded:?}",
            case.target
        );
        // `${VAR:+VAR }` contributes its separator only alongside a value, so
        // an unset RUSTFLAGS must not leave a leading space.
        ensure!(
            !expanded.starts_with(' '),
            "{} should not emit a leading separator when RUSTFLAGS is unset, \
             expanded to {expanded:?}",
            case.target
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
