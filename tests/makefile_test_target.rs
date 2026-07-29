//! Contract tests for the canonical `make test` entry point.
//!
//! `make test` is the single command local development and continuous
//! integration (CI) both run. These tests pin the runner contract it encodes:
//! non-doctest tests go through cargo-nextest, doctests run separately because
//! nextest cannot execute them, and both passes deny warnings. They also assert
//! that the checked-in nextest configuration still declares the narrow
//! serialisation group the environment-mutating suites depend on.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use toml::Value;

/// Opens the repository root as a capability-scoped directory handle.
///
/// Every read in this file is relative to that handle, so the tests cannot
/// reach outside the checkout.
fn repo_root() -> Result<Dir> {
    Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open the repository root as a capability-scoped directory")
}

fn read_repo_file(relative: &Utf8Path) -> Result<String> {
    repo_root()?
        .read_to_string(relative)
        .with_context(|| format!("{relative} should be readable"))
}

/// Splits a Make rule line into its target and its prerequisites.
///
/// Trailing `## ` help comments are discarded so `help` annotations do not leak
/// into the prerequisite list.
fn parse_rule(line: &str) -> Option<(&str, Vec<&str>)> {
    if line.starts_with(['\t', ' ', '#', '.']) {
        return None;
    }
    let (target, rest) = line.split_once(':')?;
    if target.is_empty() || rest.starts_with('=') {
        return None;
    }
    let prerequisites = rest
        .split("##")
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect();
    Some((target.trim(), prerequisites))
}

/// Returns the prerequisites declared for `target`.
fn target_prerequisites(contents: &str, target: &str) -> Option<Vec<String>> {
    contents.lines().find_map(|line| {
        let (name, prerequisites) = parse_rule(line)?;
        (name == target).then(|| prerequisites.into_iter().map(ToOwned::to_owned).collect())
    })
}

/// Returns the tab-indented recipe lines for `target`, joined by newlines.
fn target_recipe(contents: &str, target: &str) -> Option<String> {
    let mut lines = contents
        .lines()
        .skip_while(|line| parse_rule(line).is_none_or(|(name, _)| name != target));
    lines.next()?;
    let recipe: Vec<&str> = lines
        .take_while(|line| line.starts_with('\t') || line.trim().is_empty())
        .filter(|line| line.starts_with('\t'))
        .collect();
    Some(recipe.join("\n"))
}

#[test]
fn unit_parses_make_rules_and_ignores_help_comments() {
    assert_eq!(
        parse_rule("test: test-nextest doctest ## Run every Rust test"),
        Some(("test", vec!["test-nextest", "doctest"]))
    );
    assert_eq!(
        parse_rule("doctest: ## Run doctests"),
        Some(("doctest", vec![]))
    );
    assert_eq!(parse_rule("BUILD_JOBS ?="), None);
    assert_eq!(parse_rule("\tcargo nextest run"), None);
    assert_eq!(parse_rule(".PHONY: test doctest"), None);
}

#[test]
fn unit_extracts_recipe_lines_for_a_target() {
    let makefile = "test: doctest ## composite\n\ndoctest: ## docs\n\tcargo test --doc\n\techo done\n\nother:\n\ttrue\n";

    assert_eq!(target_recipe(makefile, "test").as_deref(), Some(""));
    assert_eq!(
        target_recipe(makefile, "doctest").as_deref(),
        Some("\tcargo test --doc\n\techo done")
    );
    assert_eq!(target_recipe(makefile, "missing"), None);
}

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
        nextest_recipe.contains(r#"RUSTFLAGS="-D warnings""#),
        "test-nextest should deny warnings, found {nextest_recipe:?}"
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
    for binary in ["manifest_env_tests", "ninja_env_tests", "env_path_tests"] {
        ensure!(
            filter.contains(&format!("binary({binary})")),
            "the serial-env override should cover {binary}, found filter {filter:?}"
        );
    }
    Ok(())
}
