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
//! discarding it. `test-nextest`, `doctest`, `lint-clippy`'s Clippy line,
//! `lint-whitaker`, and `typecheck` additionally add `-D warnings`;
//! `kani-full` adds only the Polonius flag, because Kani compiles third-party
//! crates the workspace lint policy does not govern. The binary-build recipe
//! and `lint-clippy`'s rustdoc line preserve the caller's value through a
//! different expansion and add neither `-D warnings` nor anything else.
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
        doctest_recipe
            .contains(r#"RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings $(POLONIUS_FLAGS)""#),
        "doctest should deny warnings and enable Polonius, found {doctest_recipe:?}"
    );
    Ok(())
}

/// `test_support` is excluded from the root workspace, so a root-level cargo
/// invocation cannot reach its tests. Each pass must therefore target its
/// manifest explicitly; dropping one of those lines silently un-gates that
/// crate, which is invisible in a green test run. `dev-test` is covered too:
/// the guide calls it the accelerated counterpart of `test-nextest`, so it
/// carries the same obligation.
///
/// Where such a line sets `RUSTFLAGS` — the `test-nextest` and `doctest`
/// passes, but not `dev-test`, which selects its toolchain instead — the value
/// is asserted by `RUSTFLAGS_CASES`, which expands each assignment rather than
/// matching recipe text.
#[rstest]
#[case::nextest("test-nextest", "nextest run")]
#[case::doctest("doctest", "--doc")]
#[case::dev_test("dev-test", "nextest run")]
fn behavioural_test_passes_also_target_test_support(
    #[case] target: &str,
    #[case] harness: &str,
) -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let recipe = target_recipe(&makefile, target)
        .with_context(|| format!("Makefile should declare a {target} target"))?;

    let scoped: Vec<&str> = recipe
        .lines()
        .filter(|line| line.contains(QUOTED_MANIFEST_FLAG))
        .collect();
    ensure!(
        scoped.len() == 1,
        concat!(
            "{target} should invoke the harness once against ",
            "$(TEST_SUPPORT_MANIFEST), found {count}: {recipe:?}",
        ),
        target = target,
        count = scoped.len(),
        recipe = recipe
    );
    for line in &scoped {
        ensure!(
            line.contains(harness),
            "{target}'s test_support pass should use {harness}, found {line:?}"
        );
        ensure!(
            line.contains("--all-features"),
            "{target}'s test_support pass should enable all features, found {line:?}"
        );
        // Checked per line rather than over the whole recipe: the root pass
        // carries the same flag, so a recipe-wide `contains` stays satisfied
        // even after the scoped pass loses it. `cargo test --doc` takes no
        // `--all-targets`, so this applies to the nextest case alone.
        if harness == "nextest run" {
            ensure!(
                line.contains("--all-targets"),
                concat!(
                    "{target}'s test_support pass should cover every test ",
                    "target, found {line:?}",
                ),
                target = target,
                line = line
            );
        }
    }
    Ok(())
}

/// Every recipe reaching `test_support` must quote the manifest variable.
///
/// `TEST_SUPPORT_MANIFEST` is overridable, so an unquoted expansion
/// word-splits on a path containing spaces and hands Cargo a truncated
/// manifest path. This reads the Makefile only — it neither runs Make nor
/// invokes Cargo — so it stays a fast static check.
#[test]
fn behavioural_test_support_passes_quote_the_manifest_variable() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let unquoted = "--manifest-path $(TEST_SUPPORT_MANIFEST)";

    let offenders: Vec<&str> = makefile
        .lines()
        .filter(|line| line.contains(unquoted) && !line.contains(QUOTED_MANIFEST_FLAG))
        .map(str::trim)
        .collect();
    ensure!(
        offenders.is_empty(),
        "every --manifest-path must quote the variable, found {offenders:#?}"
    );

    for target in ["test-nextest", "doctest", "dev-test", "lint-clippy"] {
        let recipe = target_recipe(&makefile, target)
            .with_context(|| format!("Makefile should declare a {target} target"))?;
        let quoted = recipe
            .lines()
            .filter(|line| line.contains(QUOTED_MANIFEST_FLAG))
            .count();
        ensure!(
            quoted == 1,
            concat!(
                "{target} should reach test_support on exactly one quoted ",
                "--manifest-path line, found {quoted}: {recipe:?}",
            ),
            target = target,
            quoted = quoted,
            recipe = recipe
        );
    }
    Ok(())
}

/// Splits `fragment` with `sh` and returns the resulting arguments.
///
/// The sibling `expand` helper refuses an expression containing `"`, which is
/// exactly the character under test here, so this splits rather than expands.
/// Only `sh` is involved: nothing runs Make, Cargo, or any other tool.
#[cfg(unix)]
fn shell_arguments(fragment: &str) -> Result<Vec<String>> {
    ensure!(
        !fragment.contains('\'') && !fragment.contains('`') && !fragment.contains("$("),
        "the splitting helper cannot safely embed {fragment:?}"
    );
    let script = format!("set -- {fragment}\nfor arg in \"$@\"; do printf '%s\\n' \"$arg\"; done");
    let output = Command::new("sh")
        .arg("-c")
        .arg(&script)
        .output()
        .with_context(|| format!("split {fragment:?} with sh"))?;
    ensure!(
        output.status.success(),
        "sh should split {fragment:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).context("split arguments should be UTF-8")?;
    Ok(stdout.lines().map(ToOwned::to_owned).collect())
}

/// A quoted override survives word-splitting; an unquoted one does not.
///
/// `TEST_SUPPORT_MANIFEST` is overridable, so a caller may point it at a path
/// containing spaces. Make expands the variable itself and hands the resulting
/// text to the shell, so the quotes in the recipe are what keep that path one
/// argument. `behavioural_test_support_passes_quote_the_manifest_variable`
/// pins that the Makefile really does use `QUOTED_MANIFEST_FLAG`; this checks
/// what that spelling buys.
///
/// The unquoted case is asserted too, so the test fails if the shell stops
/// discriminating rather than silently passing for the wrong reason.
#[cfg(unix)]
#[test]
fn behavioural_a_quoted_manifest_override_stays_one_argument() -> Result<()> {
    let spacey = "/tmp/netsuke contract/Cargo.toml";

    let quoted = QUOTED_MANIFEST_FLAG.replace("$(TEST_SUPPORT_MANIFEST)", spacey);
    let arguments = shell_arguments(&quoted)?;
    ensure!(
        arguments == ["--manifest-path".to_owned(), spacey.to_owned()],
        "the quoted flag should split into two arguments, got {arguments:?}"
    );

    // The control. Without it a shell that stopped splitting at all would let
    // the assertion above pass while proving nothing about the quotes.
    let unquoted = quoted.replace('"', "");
    let split = shell_arguments(&unquoted)?;
    ensure!(
        split.len() > 2,
        "an unquoted override should word-split, got {split:?}"
    );
    Ok(())
}

/// The manifest path is a `?=` variable so a caller can point the second pass
/// at a relocated crate without editing the recipes.
#[test]
fn behavioural_test_support_manifest_is_overridable() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    ensure!(
        makefile
            .lines()
            .any(|line| line.trim() == "TEST_SUPPORT_MANIFEST ?= test_support/Cargo.toml"),
        "the Makefile should default TEST_SUPPORT_MANIFEST overridably"
    );
    Ok(())
}

/// The manifest flag every `test_support` pass must carry, quoted.
///
/// The quotes are the contract, not incidental formatting: `TEST_SUPPORT_MANIFEST`
/// is overridable, so an unquoted expansion word-splits on a path containing
/// spaces and hands Cargo a truncated manifest path.
const QUOTED_MANIFEST_FLAG: &str = "--manifest-path \"$(TEST_SUPPORT_MANIFEST)\"";

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
fn behavioural_rustflags_recipes_preserve_inherited_flags(
    #[case] case: RustflagsCase,
) -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let polonius = make_variable(&makefile, "POLONIUS_FLAGS")
        .context("Makefile should define POLONIUS_FLAGS")?;
    let expanded = expand(&shell_expression(&makefile, case)?, Some(CALLER_RUSTFLAGS))?;

    ensure!(
        expanded.contains(CALLER_RUSTFLAGS),
        "{} should preserve the caller's RUSTFLAGS, expanded to {expanded:?}",
        case.target
    );
    ensure!(
        expanded.contains(&polonius),
        concat!(
            "{} should re-state {polonius} because setting RUSTFLAGS ",
            "overrides .cargo/config.toml, expanded to {expanded:?}",
        ),
        case.target,
        polonius = polonius,
        expanded = expanded
    );
    ensure!(
        expanded.contains(DENY_WARNINGS) == case.denies_warnings,
        "{} should {}deny warnings, expanded to {expanded:?}",
        case.target,
        if case.denies_warnings { "" } else { "not " }
    );
    Ok(())
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
fn behavioural_rustflags_recipes_are_well_formed_without_inherited_flags(
    #[case] case: RustflagsCase,
) -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let polonius = make_variable(&makefile, "POLONIUS_FLAGS")
        .context("Makefile should define POLONIUS_FLAGS")?;
    let expression = shell_expression(&makefile, case)?;
    let expanded = expand(&expression, None)?;

    ensure!(
        expanded.contains(&polonius),
        concat!(
            "{} should re-state {polonius} even with no inherited RUSTFLAGS, ",
            "expanded to {expanded:?}",
        ),
        case.target,
        polonius = polonius,
        expanded = expanded
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
    if case.separator_only_when_set {
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
fn behavioural_nextest_config_binds_the_env_binaries_to_the_serial_env_group() -> Result<()> {
    let config: Value = read_repo_file(&Utf8Path::new(".config").join("nextest.toml"))?
        .parse()
        .context("nextest configuration should be valid TOML")?;

    let max_threads = test_group_max_threads(&config, "serial-env")
        .context("nextest configuration should declare the serial-env test group")?;
    ensure!(
        max_threads == 1,
        "serial-env should serialize its members, found max-threads = {max_threads}"
    );

    let overrides =
        profile_overrides(&config).context("the default profile should declare overrides")?;
    let filter = overrides
        .iter()
        .filter(|entry| entry.get("test-group").and_then(Value::as_str) == Some("serial-env"))
        .find_map(|entry| entry.get("filter").and_then(Value::as_str))
        .context("an override should assign the serial-env group with a filter")?;
    for binary in ["ninja_env_tests", "env_path_tests"] {
        ensure!(
            filter.contains(&format!("binary({binary})")),
            "the serial-env override should cover {binary}, found filter {filter:?}"
        );
    }
    // A binary that no longer mutates process state must leave the group, or
    // the configuration outlives the constraint it describes and quietly
    // serializes tests that need not be. `manifest_env_tests` moved to an
    // injected reader and was removed; assert it stays out.
    ensure!(
        !filter.contains("binary(manifest_env_tests)"),
        "manifest_env_tests uses an injected reader and must not be serialized, found filter {filter:?}"
    );
    Ok(())
}
