//! Contract tests for the canonical `make test` entry point and for every
//! Makefile recipe that overrides `RUSTFLAGS`.
//!
//! `make test` is the single command local development and continuous
//! integration (CI) both run. These tests pin the runner contract it encodes:
//! non-doctest tests go through cargo-nextest, and doctests run separately
//! because nextest cannot execute them.
//!
//! They also pin the `RUSTFLAGS` contract shared by every recipe that sets the
//! variable. Setting `RUSTFLAGS` at all overrides the `[build] rustflags`
//! table in `.cargo/config.toml`, so each such recipe re-states the Polonius
//! flag, and each prepends any value the caller already exported rather than
//! discarding it. `test-nextest`, `lint-clippy`'s Clippy line, `lint-whitaker`,
//! and `typecheck` additionally add `-D warnings`;
//! `kani-full` adds only the Polonius flag, because Kani compiles third-party
//! crates the workspace lint policy does not govern. The binary-build recipe
//! preserves the caller's value through a different expansion and adds neither
//! `-D warnings` nor anything else. Rustdoc and doctests deliberately use the
//! stable warning policy without Polonius.
//!
//! The `RUSTFLAGS` tests extract each assignment from the Makefile, resolve
//! the Make variables it names, and expand the result in a shell. They assert
//! on the flags that expansion yields rather than on recipe text, so they stay
//! valid when the command a recipe runs changes. A guard test fails if a
//! recipe starts setting `RUSTFLAGS` without joining the covered set.

#[path = "support/makefile.rs"]
mod makefile;

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use makefile::{read_repo_file, target_prerequisites, target_recipe};
use rstest::rstest;
use std::collections::BTreeSet;
use std::process::Command;
use toml::Value;

#[test]
fn behavioural_make_test_composes_the_nextest_and_doctest_passes() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;

    let prerequisites =
        target_prerequisites(&makefile, "test").context("Makefile should declare a test target")?;
    for expected in ["test-nextest", "doctest"] {
        ensure!(
            prerequisites.iter().any(|name| name == expected),
            "make test should depend on {expected}, found {prerequisites:?}"
        );
    }

    let nextest_recipe = target_recipe(&makefile, "test-nextest")
        .context("Makefile should declare a test-nextest target")?;
    ensure!(
        nextest_recipe.contains("nextest run"),
        "test-nextest should route through cargo nextest run, found {nextest_recipe:?}"
    );
    ensure!(
        nextest_recipe.contains("--all-targets"),
        "test-nextest should cover every test target, found {nextest_recipe:?}"
    );
    ensure!(
        nextest_recipe.contains("--all-features"),
        "test-nextest should enable all features, found {nextest_recipe:?}"
    );
    ensure!(
        nextest_recipe
            .contains(r#"RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings $(POLONIUS_FLAGS)""#),
        "test-nextest should deny warnings and enable Polonius, found {nextest_recipe:?}"
    );

    let doctest_recipe =
        target_recipe(&makefile, "doctest").context("Makefile should declare a doctest target")?;
    ensure!(
        doctest_recipe.contains("--doc"),
        "doctest should invoke the doctest harness, found {doctest_recipe:?}"
    );
    ensure!(
        !doctest_recipe.contains("nextest"),
        "doctests cannot run under nextest, found {doctest_recipe:?}"
    );
    ensure!(
        doctest_recipe.contains(r#"RUSTFLAGS="-D warnings""#),
        "doctest should deny warnings, found {doctest_recipe:?}"
    );
    ensure!(
        doctest_recipe.contains("--workspace"),
        "doctest should cover the workspace, found {doctest_recipe:?}"
    );
    Ok(())
}

/// The prefix introducing a quoted `RUSTFLAGS` assignment in a recipe.
const RUSTFLAGS_PREFIX: &str = "RUSTFLAGS=\"";

/// A value a caller might already have exported before invoking `make`.
const CALLER_RUSTFLAGS: &str = "-C target-cpu=native";

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
    Replace,
}
/// A recipe line that overrides `RUSTFLAGS`, and the contract it must meet.
#[derive(Clone, Copy, Debug)]
struct RustflagsCase {
    /// The Make target owning the recipe.
    target: &'static str,
    /// Substring selecting the recipe line.
    line_marker: &'static str,
    /// Whether the recipe adds `-D warnings`.
    warning_policy: WarningPolicy,
    /// How the recipe handles a caller-supplied value.
    ///
    /// Polonius recipes preserve the caller either conditionally (without a
    /// leading separator when unset) or plainly. The stable doctest recipe
    /// replaces the caller's flags.
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
            inheritance_policy: InheritancePolicy::Replace,
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

    /// The per-crate Whitaker pass that loads `test_support/dylint.toml`.
    const fn lint_whitaker_test_support() -> Self {
        Self {
            target: "lint-whitaker",
            line_marker: "cd test_support",
            denies_warnings: true,
            preserves_inherited: true,
            requires_polonius: true,
            separator_only_when_set: true,
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
const RUSTFLAGS_CASES: [RustflagsCase; 8] = [
    RustflagsCase::test_nextest(),
    RustflagsCase::doctest(),
    RustflagsCase::binary_build(),
    RustflagsCase::lint_clippy(),
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
#[case(RustflagsCase::doctest())]
#[case(RustflagsCase::binary_build())]
#[case(RustflagsCase::lint_clippy())]
#[case(RustflagsCase::lint_whitaker())]
#[case(RustflagsCase::lint_whitaker_test_support())]
#[case(RustflagsCase::typecheck())]
#[case(RustflagsCase::kani_full())]
fn behavioural_rustflags_recipes_preserve_inherited_flags(
    #[case] case: RustflagsCase,
) -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let polonius = make_variable(&makefile, "POLONIUS_FLAGS")
        .context("Makefile should define POLONIUS_FLAGS")?;
    let expanded = expand(&shell_expression(&makefile, case)?, Some(CALLER_RUSTFLAGS))?;

    ensure!(
        expanded.contains(CALLER_RUSTFLAGS)
            == (case.inheritance_policy != InheritancePolicy::Replace),
        "{} inherited-RUSTFLAGS contract should hold, expanded to {expanded:?}",
        case.target
    );
    ensure!(
        expanded.contains(&polonius) == (case.inheritance_policy != InheritancePolicy::Replace),
        "{} Polonius contract should hold for {polonius}, expanded to {expanded:?}",
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
    Ok(())
}

#[cfg(unix)]
#[rstest]
#[case(RustflagsCase::test_nextest())]
#[case(RustflagsCase::doctest())]
#[case(RustflagsCase::binary_build())]
#[case(RustflagsCase::lint_clippy())]
#[case(RustflagsCase::lint_whitaker())]
#[case(RustflagsCase::lint_whitaker_test_support())]
#[case(RustflagsCase::typecheck())]
#[case(RustflagsCase::kani_full())]
fn behavioural_rustflags_recipes_are_well_formed_without_inherited_flags(
    #[case] case: RustflagsCase,
) -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let polonius = make_variable(&makefile, "POLONIUS_FLAGS")
        .context("Makefile should define POLONIUS_FLAGS")?;
    let expression = shell_expression(&makefile, case)?;
    let expanded = expand(&expression, None)?;

    ensure!(
        expanded.contains(&polonius) == (case.inheritance_policy != InheritancePolicy::Replace),
        "{} Polonius contract should hold for {polonius}, expanded to {expanded:?}",
        case.target
    );
    ensure!(
        !expanded.contains(CALLER_RUSTFLAGS),
        "{} should not invent flags the caller never set, expanded to {expanded:?}",
        case.target
    );
    // `${VAR:+VAR }` contributes its separator only alongside a value, so an
    // unset RUSTFLAGS must not leave a leading space. Recipes spelling the
    // expansion `${VAR-}` tolerate one, so the case declares which contract
    // applies. This is what separates the idiom from a bare `$RUSTFLAGS `
    // prefix, which preserves the caller's flags but strands a separator.
    if case.inheritance_policy == InheritancePolicy::Conditional {
        ensure!(
            !expanded.starts_with(' '),
            concat!(
                "{} should not emit a leading separator when RUSTFLAGS is ",
                "unset, expanded to {expanded:?}",
            ),
            case.target,
            expanded = expanded
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

/// Returns the `[[profile.default.overrides]]` entries.
fn profile_overrides(config: &Value) -> Option<&[Value]> {
    config
        .get("profile")?
        .get("default")?
        .get("overrides")?
        .as_array()
        .map(Vec::as_slice)
}

/// Returns the `max-threads` declared for a named test group.
fn test_group_max_threads(config: &Value, group: &str) -> Option<i64> {
    config
        .get("test-groups")?
        .get(group)?
        .get("max-threads")?
        .as_integer()
}

#[test]
fn behavioural_nextest_config_does_not_serialize_environment_tests() -> Result<()> {
    let config: Value = read_repo_file(&Utf8Path::new(".config").join("nextest.toml"))?
        .parse()
        .context("nextest configuration should be valid TOML")?;

    ensure!(
        test_group_max_threads(&config, "serial-env").is_none(),
        "environment tests use injected state and should not declare a serial-env test group"
    );

    let has_serial_override = profile_overrides(&config).is_some_and(|overrides| {
        overrides
            .iter()
            .any(|entry| entry.get("test-group").and_then(Value::as_str) == Some("serial-env"))
    });
    ensure!(
        !has_serial_override,
        "environment tests use injected state and should not have a serial-env override"
    );
    Ok(())
}
