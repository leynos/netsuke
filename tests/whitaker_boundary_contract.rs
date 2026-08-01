//! Contract tests pinning the `no_std_fs_operations` exclusion boundary.
//!
//! The capability policy is enforced by configuration rather than by code, so
//! a regression here is silent: widening an entry from a module path to a whole
//! crate, or dropping the second Whitaker run, stops the lint reporting real
//! violations without failing anything. These tests pin the two invariants that
//! keep the boundary honest.
//!
//! First, `test_support` is excluded from the Cargo workspace, so a
//! workspace-root `cargo dylint` cannot reach it. `make lint-whitaker`
//! therefore runs the suite a second time from `test_support/`, against that
//! crate's own `dylint.toml`. Without the second invocation the crate is
//! unlinted while still appearing to be covered.
//!
//! Second, every exemption must name a bounded module. A bare crate name in
//! `excluded_paths`, or the application crate reappearing in `excluded_crates`,
//! would exempt far more than the ambient boundary it was added for.
//!
//! These assertions are deterministic file checks. They do not invoke Whitaker:
//! the suite needs its own pinned toolchain and driver, which `make test` does
//! not require. `make lint-whitaker` remains the gate that actually runs it,
//! and `docs/developers-guide.md` records the manual negative probe for
//! confirming the exclusions have not widened.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::rstest;
use toml::Value as TomlValue;

/// The one module in `test_support` permitted to touch ambient `std::fs`.
const TEST_SUPPORT_BOUNDARY: &str = "test_support::fs";

/// Opens the repository root as a capability-scoped directory handle.
fn repo_root() -> Result<Dir> {
    Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open the repository root as a capability-scoped directory")
}

fn read_repo_file(relative: &Utf8Path) -> Result<String> {
    repo_root()?
        .read_to_string(relative)
        .with_context(|| format!("{relative} should be readable"))
}

/// Returns the tab-indented recipe lines for `target`, joined by newlines.
fn target_recipe(contents: &str, target: &str) -> Option<String> {
    let mut lines = contents.lines().skip_while(|line| {
        line.starts_with(['\t', ' ', '#', '.'])
            || line
                .split_once(':')
                .is_none_or(|(name, rest)| name.trim() != target || rest.starts_with('='))
    });
    lines.next()?;
    let recipe: Vec<&str> = lines
        .take_while(|line| line.starts_with('\t') || line.trim().is_empty())
        .filter(|line| line.starts_with('\t'))
        .collect();
    Some(recipe.join("\n"))
}

/// Returns the string entries of `key` under `[no_std_fs_operations]`.
fn exclusion_list(dylint_toml: &str, key: &str) -> Result<Vec<String>> {
    let config: TomlValue = dylint_toml.parse().context("parse dylint.toml")?;
    let Some(entries) = config
        .get("no_std_fs_operations")
        .and_then(|lint| lint.get(key))
    else {
        return Ok(Vec::new());
    };
    let array = entries
        .as_array()
        .with_context(|| format!("{key} should be an array"))?;
    array
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_owned)
                .with_context(|| format!("{key} entries should be strings, found {entry:?}"))
        })
        .collect()
}

#[test]
fn lint_whitaker_also_runs_inside_test_support() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let recipe = target_recipe(&makefile, "lint-whitaker")
        .context("the Makefile should declare a lint-whitaker target")?;

    let invocations: Vec<&str> = recipe
        .lines()
        .filter(|line| line.contains("$(WHITAKER)"))
        .collect();
    ensure!(
        invocations.len() == 2,
        "lint-whitaker should invoke Whitaker twice — once at the repository \
         root and once inside test_support, which the workspace excludes — \
         found {count}: {recipe:?}",
        count = invocations.len()
    );
    ensure!(
        invocations
            .iter()
            .any(|line| line.contains("cd test_support")),
        "one lint-whitaker invocation should run from test_support/: {recipe:?}"
    );
    Ok(())
}

#[test]
fn test_support_carries_its_own_scoped_lint_config() -> Result<()> {
    let config = read_repo_file(Utf8Path::new("test_support/dylint.toml"))?;

    let paths = exclusion_list(&config, "excluded_paths")?;
    ensure!(
        paths == [TEST_SUPPORT_BOUNDARY],
        "test_support/dylint.toml should exempt only {TEST_SUPPORT_BOUNDARY}, found {paths:?}"
    );

    let crates = exclusion_list(&config, "excluded_crates")?;
    ensure!(
        crates.is_empty(),
        "test_support should not exempt whole crates; the boundary is the \
         {TEST_SUPPORT_BOUNDARY} module, found {crates:?}"
    );
    Ok(())
}

#[rstest]
#[case::root("dylint.toml")]
#[case::test_support("test_support/dylint.toml")]
fn excluded_paths_name_bounded_modules(#[case] relative: &str) -> Result<()> {
    let paths = exclusion_list(&read_repo_file(Utf8Path::new(relative))?, "excluded_paths")?;
    for path in &paths {
        ensure!(
            path.contains("::"),
            "{relative}: {path:?} names a whole crate; excluded_paths entries \
             must name a module so siblings stay under the capability policy"
        );
    }
    Ok(())
}

#[rstest]
#[case::application_crate("netsuke")]
#[case::test_support("test_support")]
fn crate_is_not_exempted_wholesale(#[case] crate_name: &str) -> Result<()> {
    let crates = exclusion_list(
        &read_repo_file(Utf8Path::new("dylint.toml"))?,
        "excluded_crates",
    )?;
    ensure!(
        !crates.iter().any(|entry| entry == crate_name),
        "{crate_name} should not appear in excluded_crates: its ambient access \
         is scoped to named modules, and test_support is linted separately \
         against test_support/dylint.toml"
    );
    Ok(())
}

#[test]
fn runner_file_io_is_scoped_to_the_ambient_sync_submodule() -> Result<()> {
    let paths = exclusion_list(
        &read_repo_file(Utf8Path::new("dylint.toml"))?,
        "excluded_paths",
    )?;
    ensure!(
        paths
            .iter()
            .any(|path| path == "netsuke::runner::process::file_io::ambient_sync"),
        "the runner's durability sync should be exempt only in its \
         ambient_sync submodule, found {paths:?}"
    );
    ensure!(
        !paths
            .iter()
            .any(|path| path == "netsuke::runner::process::file_io"),
        "exempting the whole file_io module would also exempt its \
         capability-based write helpers, found {paths:?}"
    );
    Ok(())
}
