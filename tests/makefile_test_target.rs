//! Contract tests for the canonical `make test` entry point and for every
//! Makefile recipe that overrides `RUSTFLAGS`.
//!
//! `make test` is the single command local development and continuous
//! integration (CI) both run. These tests pin the runner contract it encodes:
//! the mutation gate completes before non-doctest tests go through
//! cargo-nextest and doctests run separately because nextest cannot execute
//! them.
//!
//! They also pin the `RUSTFLAGS` contract shared by every recipe that sets the
//! variable. Each such recipe adds `-D warnings` and prepends any value the
//! caller already exported rather than discarding it. `kani-full`,
//! `bench-config-load`, and the binary-build recipe deliberately set nothing:
//! Kani compiles third-party crates the workspace lint policy does not govern,
//! and neither a benchmark nor a plain binary build is a lint gate.
//!
//! The `RUSTFLAGS` tests extract each assignment from the Makefile and expand
//! it in a shell. They assert on the flags that expansion yields rather than
//! on recipe text, so they stay valid when the command a recipe runs changes.
//! A guard test fails if a recipe starts setting `RUSTFLAGS` without joining
//! the covered set.

#[path = "support/makefile.rs"]
mod makefile;

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use makefile::{phony_targets, read_repo_file, target_prerequisites, target_recipe};
use toml::Value;

#[test]
fn behavioural_make_test_stages_the_mutation_gate_before_rust_tests() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;

    let prerequisites =
        target_prerequisites(&makefile, "test").context("Makefile should declare a test target")?;
    ensure!(
        prerequisites == ["test-env-mutation-gate"],
        "make test must stage the mutation gate before its recursive test pass, found {prerequisites:?}"
    );
    let test_recipe =
        target_recipe(&makefile, "test").context("Makefile should declare a test recipe")?;
    ensure!(
        test_recipe.contains("$(MAKE)")
            && test_recipe.contains("test-nextest")
            && test_recipe.contains("doctest"),
        "make test must run nextest and doctests through a staged recursive make, found {test_recipe:?}"
    );

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
        nextest_recipe.contains("--workspace"),
        "test-nextest should cover the workspace, found {nextest_recipe:?}"
    );
    ensure!(
        nextest_recipe.contains(r#"RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings""#),
        "test-nextest should preserve inherited flags and deny warnings, found {nextest_recipe:?}"
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
        doctest_recipe.contains(r#"RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }-D warnings""#),
        "doctest should preserve inherited flags and deny warnings; found {doctest_recipe:?}"
    );
    ensure!(
        doctest_recipe.contains("--workspace"),
        "doctest should cover the workspace, found {doctest_recipe:?}"
    );
    Ok(())
}

#[test]
fn mutation_gate_targets_are_phony_make_prerequisites() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let phony = phony_targets(&makefile);

    for target in ["test-env-mutation-gate", "lint-env-mutation"] {
        ensure!(
            phony.contains(&target),
            ".PHONY must include {target}, found {phony:?}"
        );
    }
    for (target, prerequisite) in [
        ("test", "test-env-mutation-gate"),
        ("lint", "lint-env-mutation"),
    ] {
        let prerequisites = target_prerequisites(&makefile, target)
            .with_context(|| format!("Makefile should declare a {target} target"))?;
        ensure!(
            prerequisites.iter().any(|name| name == prerequisite),
            "make {target} should depend on {prerequisite}, found {prerequisites:?}"
        );
    }
    Ok(())
}

#[cfg(unix)]
#[path = "makefile_test_target/mutation_gate.rs"]
mod mutation_gate;
#[path = "makefile_test_target/rustflags.rs"]
mod rustflags;

/// Returns every nextest profile override.
fn all_profile_overrides(config: &Value) -> impl Iterator<Item = &Value> {
    config
        .get("profile")
        .and_then(Value::as_table)
        .into_iter()
        .flat_map(toml::map::Map::values)
        .filter_map(|profile| profile.get("overrides").and_then(Value::as_array))
        .flatten()
}

#[test]
fn behavioural_nextest_config_does_not_serialize_environment_tests() -> Result<()> {
    let config: Value = read_repo_file(&Utf8Path::new(".config").join("nextest.toml"))?
        .parse()
        .context("nextest configuration should be valid TOML")?;

    ensure!(
        config
            .get("test-groups")
            .and_then(|groups| groups.get("serial-env"))
            .is_none(),
        "environment tests use injected state and should not declare a serial-env test group"
    );

    let has_serial_override = all_profile_overrides(&config)
        .any(|entry| entry.get("test-group").and_then(Value::as_str) == Some("serial-env"));
    ensure!(
        !has_serial_override,
        "environment tests use injected state and should not have a serial-env override"
    );
    Ok(())
}
