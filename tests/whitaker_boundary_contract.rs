//! Contract tests pinning the `no_std_fs_operations` exclusion boundary.
//!
//! The capability policy is enforced by configuration rather than by code, so
//! a regression here is silent: widening an entry from a module path to a whole
//! crate, or dropping the second Whitaker run, stops the lint reporting real
//! violations without failing anything. These tests pin the two invariants that
//! keep the boundary honest.
//!
//! First, `test_support` is a Cargo workspace member, but its sanctioned
//! boundary is configured in its own `dylint.toml`. The root invocation
//! therefore selects only the `netsuke` package and tells Dylint not to check
//! dependencies, while `make lint-whitaker` runs the suite a second time from
//! `test_support/`. Without that package boundary, the root policy would
//! override the crate-specific exemption; without the second invocation, the
//! crate would be unlinted.
//!
//! Second, every exemption must name a bounded module. A bare crate name in
//! `excluded_paths`, or the application crate reappearing in `excluded_crates`,
//! would exempt far more than the ambient boundary it was added for.
//!
//! Third, neither manifest may pin the lint-library source. Installing the
//! libraries at Whitaker HEAD is a decision, not an oversight in the installer
//! pin, and it has been proposed as a defect more than once. A
//! `[workspace.metadata.dylint]` block would quietly reverse it, so the absence
//! is asserted here rather than left to review.
//!
//! These assertions are deterministic file checks. They do not invoke Whitaker:
//! the suite needs its own pinned toolchain and driver, which `make test` does
//! not require. `make lint-whitaker` remains the gate that actually runs it,
//! and `docs/developers-guide.md` records the manual negative probe for
//! confirming the exclusions have not widened.

#[path = "support/makefile.rs"]
mod makefile;

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use makefile::{read_repo_file, target_recipe};
use rstest::rstest;
use toml::Value as TomlValue;

/// The one module in `test_support` permitted to touch ambient `std::fs`.
const TEST_SUPPORT_BOUNDARY: &str = "test_support::fs";

/// PATH resolution for the `which` standard library function, whose candidate
/// directories come from the ambient `PATH`.
const WHICH_LOOKUP_BOUNDARY: &str = "netsuke::stdlib::which::lookup";

/// Durability sync for the runner's temporary Ninja file, scoped to the
/// submodule holding only that `sync_all`.
const RUNNER_SYNC_BOUNDARY: &str = "netsuke::runner::process::file_io::ambient_sync";

/// Manifest tables that can carry a `metadata.dylint` block, either of which
/// Dylint would honour to resolve lint libraries from a pinned git source.
const METADATA_TABLES: [&str; 2] = ["workspace", "package"];

/// Neither manifest may pin `whitaker_suite` to a tag or revision.
///
/// Netsuke installs the lint libraries at Whitaker HEAD through
/// `whitaker-installer`, which stages them from the suite's default branch.
/// `WHITAKER_INSTALLER_VERSION` pins the installer binary — a separate artefact
/// that says nothing about which lints are staged. A `metadata.dylint` block
/// would take over library resolution and freeze lint behaviour at whatever
/// revision it names, which is the opposite of the intended policy.
///
/// Keyed on the `dylint` entry rather than on `metadata`, because the root
/// manifest legitimately carries `package.metadata` for other tools.
#[rstest]
#[case::root("Cargo.toml")]
#[case::test_support("test_support/Cargo.toml")]
fn manifests_do_not_pin_the_lint_libraries(#[case] relative: &str) -> Result<()> {
    let manifest: TomlValue = read_repo_file(Utf8Path::new(relative))?
        .parse()
        .with_context(|| format!("parse {relative}"))?;

    for table in METADATA_TABLES {
        let pinned = manifest
            .get(table)
            .and_then(|section| section.get("metadata"))
            .and_then(|metadata| metadata.get("dylint"));
        ensure!(
            pinned.is_none(),
            concat!(
                "{relative} declares [{table}.metadata.dylint], which would pin ",
                "the lint libraries to a fixed source. This repository installs ",
                "them at Whitaker HEAD on purpose; WHITAKER_INSTALLER_VERSION ",
                "pins the installer, not the libraries. Read the quality-gates ",
                "section of docs/developers-guide.md before changing this.",
            ),
            relative = relative,
            table = table
        );
    }
    Ok(())
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
        concat!(
            "lint-whitaker should invoke Whitaker twice — once at the ",
            "repository root for netsuke and once inside test_support — ",
            "found {count}: {recipe:?}",
        ),
        count = invocations.len(),
        recipe = recipe
    );

    // Counted separately rather than with `any`, which two test_support runs
    // would satisfy while leaving the whole application crate unlinted.
    let (scoped, root): (Vec<&str>, Vec<&str>) = invocations
        .iter()
        .partition(|line| line.contains("cd test_support"));
    ensure!(
        scoped.len() == 1,
        concat!(
            "exactly one lint-whitaker invocation should run from ",
            "test_support/, found {count}: {recipe:?}",
        ),
        count = scoped.len(),
        recipe = recipe
    );
    ensure!(
        scoped.first().is_some_and(|line| {
            line.contains("DYLINT_TOML")
                && line.contains("cat dylint.toml")
                && line.contains("--package test_support")
                && line.contains("--no-deps")
        }),
        concat!(
            "the scoped Whitaker invocation must explicitly load ",
            "test_support/dylint.toml and select only test_support after ",
            "workspace membership changes Cargo's configuration and package ",
            "roots; found {scoped:?}",
        ),
        scoped = format!("{scoped:?}")
    );
    ensure!(
        root.len() == 1,
        concat!(
            "exactly one lint-whitaker invocation should run from the ",
            "repository root, found {count}: {recipe:?}",
        ),
        count = root.len(),
        recipe = recipe
    );
    ensure!(
        root.first().is_some_and(|line| {
            line.contains("--package netsuke") && line.contains("--no-deps")
        }),
        concat!(
            "the root Whitaker invocation must select only netsuke and skip ",
            "dependency checks so test_support loads its own dylint.toml; ",
            "found {root:?}",
        ),
        root = format!("{root:?}")
    );
    Ok(())
}

#[test]
fn test_support_carries_its_own_scoped_lint_config() -> Result<()> {
    let config = read_repo_file(Utf8Path::new("test_support/dylint.toml"))?;

    // The exact set, not mere membership. `test_support::fs` is the crate's
    // only sanctioned ambient boundary, and the exemption list is meant to stay
    // one entry long: the `dev_fast` modules that once looked like they needed
    // their own entries were all expressible as wrappers returning plain data
    // (see the rationale in `test_support/dylint.toml`). A second entry is
    // therefore a decision to review, not a routine addition, so it should fail
    // here and be justified rather than slip in unnoticed.
    let paths = exclusion_list(&config, "excluded_paths")?;
    ensure!(
        paths == [TEST_SUPPORT_BOUNDARY],
        concat!(
            "test_support/dylint.toml should exempt exactly {boundary} and ",
            "nothing else; adding an entry needs the same scrutiny that ",
            "removing the dev_fast ones did. Found {paths:?}",
        ),
        boundary = TEST_SUPPORT_BOUNDARY,
        paths = format!("{paths:?}")
    );

    let crates = exclusion_list(&config, "excluded_crates")?;
    ensure!(
        crates.is_empty(),
        concat!(
            "test_support should not exempt whole crates; the boundary is ",
            "the {boundary} module, found {crates:?}",
        ),
        boundary = TEST_SUPPORT_BOUNDARY,
        crates = format!("{crates:?}")
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
            concat!(
                "{relative}: {path:?} names a whole crate; excluded_paths ",
                "entries must name a module so siblings stay under the ",
                "capability policy",
            ),
            relative = relative,
            path = path
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
        concat!(
            "{crate_name} should not appear in excluded_crates: its ambient ",
            "access is scoped to named modules, and test_support is linted ",
            "separately against test_support/dylint.toml",
        ),
        crate_name = crate_name
    );
    Ok(())
}

/// Production boundaries that must stay exempt, each paired with the wider path
/// that must not appear because it would also exempt capability-based siblings.
///
/// Membership rather than an exact set: `excluded_paths` legitimately grows as
/// new ambient boundaries are identified — `netsuke::cli::discovery::paths` was
/// added upstream while this branch was in review — and pinning the whole list
/// would turn every reviewed addition into a test failure without catching any
/// widening of the entries that matter.
#[rstest]
#[case::which_lookup(WHICH_LOOKUP_BOUNDARY, "netsuke::stdlib::which")]
#[case::runner_ambient_sync(RUNNER_SYNC_BOUNDARY, "netsuke::runner::process::file_io")]
fn production_boundaries_stay_narrowly_scoped(
    #[case] required: &str,
    #[case] widened: &str,
) -> Result<()> {
    let paths = exclusion_list(
        &read_repo_file(Utf8Path::new("dylint.toml"))?,
        "excluded_paths",
    )?;
    ensure!(
        paths.iter().any(|path| path == required),
        "{required} should be exempt, found {paths:?}"
    );
    ensure!(
        !paths.iter().any(|path| path == widened),
        concat!(
            "{widened} would exempt more than the ambient boundary it was ",
            "added for; keep the entry at {required}, found {paths:?}",
        ),
        widened = widened,
        required = required,
        paths = format!("{paths:?}")
    );
    Ok(())
}

/// Does `entry` exempt `path` under Whitaker's segment-boundary matching?
///
/// An entry covers itself and its descendants, but never a sibling that merely
/// shares a textual prefix. This mirrors the rule documented in
/// `docs/whitaker-users-guide.md` and is the reason every entry above is a
/// module path: it is what makes `netsuke::stdlib::which::lookup` safe to
/// exempt without also exempting `netsuke::stdlib::which::cache`.
fn entry_covers(entry: &str, path: &str) -> bool {
    path == entry
        || path
            .strip_prefix(entry)
            .is_some_and(|r| r.starts_with("::"))
}

proptest::proptest! {
    /// A descendant of an exempt module is covered, however deeply nested.
    #[test]
    fn exemptions_cover_their_descendants(
        segments in proptest::collection::vec("[a-z][a-z0-9_]{0,7}", 1..4),
    ) {
        let entry = WHICH_LOOKUP_BOUNDARY;
        let descendant = format!("{entry}::{}", segments.join("::"));
        proptest::prop_assert!(
            entry_covers(entry, &descendant),
            "{entry} should cover {descendant}"
        );
    }

    /// A sibling sharing a textual prefix is never covered. This is the case a
    /// crate-wide exclusion would silently swallow.
    #[test]
    fn exemptions_never_cover_prefix_siblings(
        suffix in "[a-z0-9_]{1,8}",
    ) {
        let entry = WHICH_LOOKUP_BOUNDARY;
        let sibling = format!("{entry}{suffix}");
        proptest::prop_assert!(
            !entry_covers(entry, &sibling),
            "{entry} must not cover the sibling {sibling}"
        );
    }

    /// Truncating an entry to a shorter module prefix widens it: the prefix
    /// covers the original entry, which is why entries are kept at the
    /// narrowest module that owns the ambient operation.
    #[test]
    fn shorter_prefixes_are_strictly_wider(
        depth in 1usize..4,
    ) {
        let entry = RUNNER_SYNC_BOUNDARY;
        let segments: Vec<&str> = entry.split("::").collect();
        let keep = segments.len().saturating_sub(depth).max(1);
        let prefix = segments
            .iter()
            .take(keep)
            .copied()
            .collect::<Vec<&str>>()
            .join("::");
        proptest::prop_assert!(
            entry_covers(&prefix, entry),
            "{prefix} would cover {entry}, so it is the wider exemption"
        );
        proptest::prop_assert!(
            !entry_covers(entry, &prefix),
            "{entry} must not cover its own ancestor {prefix}"
        );
    }
}
