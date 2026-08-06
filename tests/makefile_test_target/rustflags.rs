//! Contract model for Makefile recipes that set `RUSTFLAGS`.

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WarningPolicy {
    Deny,
    Default,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InheritancePolicy {
    Conditional,
    Plain,
}

/// A recipe line that overrides `RUSTFLAGS`, and the contract it must meet.
#[derive(Clone, Copy, Debug)]
struct RustflagsCase {
    /// The Make target owning the recipe.
    target: &'static str,
    /// Substring selecting the recipe line.
    line_marker: &'static str,
    /// Whether the recipe adds `-D warnings`.
    ///
    /// Only the Unix behavioural tests read the policy fields (expansion
    /// needs a shell), so non-Unix builds would otherwise flag them dead.
    #[cfg_attr(
        not(unix),
        expect(dead_code, reason = "read only by Unix expansion tests")
    )]
    warning_policy: WarningPolicy,
    /// How the recipe handles a caller-supplied value.
    #[cfg_attr(
        not(unix),
        expect(dead_code, reason = "read only by Unix expansion tests")
    )]
    inheritance_policy: InheritancePolicy,
}

impl RustflagsCase {
    const fn test_nextest() -> Self {
        Self {
            target: "test-nextest",
            line_marker: "nextest run",
            warning_policy: WarningPolicy::Deny,
            inheritance_policy: InheritancePolicy::Conditional,
        }
    }

    const fn doctest() -> Self {
        Self {
            target: "doctest",
            line_marker: "--doc",
            warning_policy: WarningPolicy::Deny,
            inheritance_policy: InheritancePolicy::Conditional,
        }
    }

    const fn binary_build() -> Self {
        Self {
            target: "target/%/$(APP)",
            line_marker: "build",
            warning_policy: WarningPolicy::Default,
            inheritance_policy: InheritancePolicy::Plain,
        }
    }

    const fn lint_rustdoc() -> Self {
        Self {
            target: "lint-clippy",
            line_marker: "doc --workspace",
            warning_policy: WarningPolicy::Deny,
            inheritance_policy: InheritancePolicy::Conditional,
        }
    }

    const fn lint_clippy() -> Self {
        Self {
            target: "lint-clippy",
            line_marker: "clippy",
            warning_policy: WarningPolicy::Deny,
            inheritance_policy: InheritancePolicy::Conditional,
        }
    }

    const fn lint_whitaker() -> Self {
        Self {
            target: "lint-whitaker",
            line_marker: "$(WHITAKER)",
            warning_policy: WarningPolicy::Deny,
            inheritance_policy: InheritancePolicy::Conditional,
        }
    }

    const fn lint_whitaker_test_support() -> Self {
        Self {
            target: "lint-whitaker",
            line_marker: "cd test_support",
            warning_policy: WarningPolicy::Deny,
            inheritance_policy: InheritancePolicy::Conditional,
        }
    }

    const fn typecheck() -> Self {
        Self {
            target: "typecheck",
            line_marker: "check",
            warning_policy: WarningPolicy::Deny,
            inheritance_policy: InheritancePolicy::Conditional,
        }
    }

    const fn kani_full() -> Self {
        Self {
            target: "kani-full",
            line_marker: "$(KANI)",
            warning_policy: WarningPolicy::Default,
            inheritance_policy: InheritancePolicy::Conditional,
        }
    }
}

/// Every `RUSTFLAGS`-setting recipe line under contract.
const RUSTFLAGS_CASES: [RustflagsCase; 9] = [
    RustflagsCase::test_nextest(),
    RustflagsCase::doctest(),
    RustflagsCase::binary_build(),
    RustflagsCase::lint_rustdoc(),
    RustflagsCase::lint_clippy(),
    RustflagsCase::lint_whitaker(),
    RustflagsCase::lint_whitaker_test_support(),
    RustflagsCase::typecheck(),
    RustflagsCase::kani_full(),
];

/// Resolves `POLONIUS_FLAGS`, rejecting a missing or empty definition.
///
/// Every assertion built on the resolved value uses `contains`, which an
/// empty string satisfies vacuously, so an empty definition would silently
/// void the Polonius contract rather than fail it.
fn polonius_flags(makefile: &str) -> Result<String> {
    let value = make_variable(makefile, "POLONIUS_FLAGS")
        .context("Makefile should define POLONIUS_FLAGS")?;
    ensure!(
        !value.is_empty(),
        "POLONIUS_FLAGS should not be empty; the contract cannot assert a vacuous flag"
    );
    Ok(value)
}

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
    let polonius = polonius_flags(makefile)?;
    let resolved = assignment
        .replace("$(POLONIUS_FLAGS)", &polonius)
        .replace("$$", "$");
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
    let makefile = concat!(
        "POLONIUS_FLAGS ?= -Zpolonius=next\n",
        "unsafe-recipe:\n",
        "\tRUSTFLAGS=\"$$(date) $(POLONIUS_FLAGS)\" echo unsafe\n",
    );
    let case = RustflagsCase {
        target: "unsafe-recipe",
        line_marker: "echo unsafe",
        warning_policy: WarningPolicy::Default,
        inheritance_policy: InheritancePolicy::Conditional,
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
    let polonius = polonius_flags(&makefile)?;
    for case in RUSTFLAGS_CASES {
        let expanded = expand(&shell_expression(&makefile, case)?, Some(CALLER_RUSTFLAGS))?;

        ensure!(
            expanded.contains(CALLER_RUSTFLAGS),
            "{} inherited-RUSTFLAGS contract should hold, expanded to {expanded:?}",
            case.target
        );
        ensure!(
            expanded.contains(&polonius),
            "{} should enable Polonius with {polonius}, expanded to {expanded:?}",
            case.target
        );
        ensure!(
            expanded.contains(DENY_WARNINGS) == (case.warning_policy == WarningPolicy::Deny),
            "{} should {}deny warnings, expanded to {expanded:?}",
            case.target,
            if case.warning_policy == WarningPolicy::Deny {
                ""
            } else {
                "not "
            }
        );
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn behavioural_rustflags_recipes_are_well_formed_without_inherited_flags() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let polonius = polonius_flags(&makefile)?;
    for case in RUSTFLAGS_CASES {
        let expression = shell_expression(&makefile, case)?;
        let expanded = expand(&expression, None)?;

        ensure!(
            expanded.contains(&polonius),
            "{} should enable Polonius with {polonius}, expanded to {expanded:?}",
            case.target
        );
        ensure!(
            !expanded.contains(CALLER_RUSTFLAGS),
            "{} should not invent flags the caller never set, expanded to {expanded:?}",
            case.target
        );
        // `${VAR:+VAR }` contributes its separator only alongside a value, so
        // an unset RUSTFLAGS must not leave a leading space. Recipes spelling
        // `${VAR-}` tolerate one, so the case declares which contract applies.
        if case.inheritance_policy == InheritancePolicy::Conditional {
            ensure!(
                !expanded.starts_with(' '),
                "{} should not emit a leading separator when RUSTFLAGS is unset, \
                 expanded to {expanded:?}",
                case.target
            );
        }
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
