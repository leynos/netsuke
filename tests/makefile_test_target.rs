//! Contract tests for the canonical `make test` entry point.
//!
//! `make test` is the single command local development and continuous
//! integration (CI) both run. These tests pin the runner contract it encodes:
//! non-doctest tests go through cargo-nextest, and doctests run separately
//! because nextest cannot execute them.
//!
//! The companion `RUSTFLAGS` contract — shared by every recipe that sets the
//! variable — lives in the `rustflags_contract` module.

#[path = "support/makefile.rs"]
mod makefile;
#[path = "makefile_test_target/rustflags_contract.rs"]
mod rustflags_contract;

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use makefile::{read_repo_file, target_prerequisites, target_recipe};
use rstest::rstest;
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
    // Exact membership, not merely presence. Checking only that the expected
    // binaries appear would let a third join the group unnoticed, silently
    // serializing tests that need not be — and would leave the developers'
    // guide's "exactly two" claim unenforced. Equally, a binary that stops
    // mutating process state must leave, or the configuration outlives the
    // constraint it describes; `manifest_env_tests` did exactly that when it
    // moved to an injected reader.
    let mut members: Vec<&str> = filter
        .split("binary(")
        .skip(1)
        .filter_map(|rest| rest.split(')').next())
        .collect();
    members.sort_unstable();
    ensure!(
        members == ["env_path_tests", "ninja_env_tests"],
        "serial-env should cover exactly the two PATH- and NINJA_ENV-mutating \
         binaries; found {members:?} in filter {filter:?}"
    );
    Ok(())
}
