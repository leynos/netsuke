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
//! preserves the caller's value through a different expansion and adds
//! Polonius, but not `-D warnings`. Rustdoc and doctests inherit caller flags,
//! deny warnings, and explicitly restore Polonius like the other checked
//! recipes.
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
            .contains(r#"RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings $(POLONIUS_FLAGS)""#,),
        "doctest should preserve inherited flags, deny warnings, and enable Polonius; found {doctest_recipe:?}"
    );
    ensure!(
        doctest_recipe.contains("--workspace"),
        "doctest should cover the workspace, found {doctest_recipe:?}"
    );
    Ok(())
}

#[path = "makefile_test_target/rustflags.rs"]
mod rustflags;

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
