//! What the crate-internal manifest-query entry point produces today.
//!
//! `from_path_for_manifest_query_with_limits` is the loader behind
//! `netsuke help targets`, and it is crate-private, so the public suite in
//! `tests/manifest_load_options/wrapper_equivalence.rs` cannot reach it. It
//! needs its own characterization for the same reason the public entries do:
//! the refactor that replaces the shared loader core must leave it producing
//! exactly what it produces now, and the pre-refactor code is the only
//! trustworthy oracle. Comparing it with the core afterwards would be equal by
//! construction.
//!
//! # Why this row cannot reuse the public fixtures
//!
//! The public suite's fixtures live in the `tests/` integration crate, which
//! `src/` cannot import. The corpus and its staging are therefore restated
//! here — deliberately not shared — and the manifests are staged inline rather
//! than read from `tests/data`.
//!
//! That restatement is affordable because this entry point admits a small
//! share of the public corpus. It builds its own environment from
//! `disabled_env_reader`, so no fixture that calls `env()` is loadable; it
//! registers the query surface, so `fetch`, `shell`, and `glob` are refused
//! rather than exercised; and it takes no `StdlibConfig`, so no clock can be
//! injected and `now()` is a refusal too. What remains is exactly what this
//! module stages: a pure manifest, the malformed-YAML path, the Boolean
//! interpolation that separates the query surface from the build surface, and
//! the budget-exhaustion path.
//!
//! # Naming and the move to EP-M1
//!
//! The snapshot is named `from_path_with_limits` rather than after this
//! module, and the settings drop insta's module prefix, so the file's name and
//! its directory depend only on what is being asserted. EP-M1 moves the
//! entry point's body into `src/manifest/query.rs`; a name carrying this
//! module's path would change with the move and read as a behaviour change.

use crate::manifest::{self, ManifestBudgetLimits};
use crate::snapshot_test_support::stable_snapshot_settings;
use anyhow::{Context, Result};
use rstest::rstest;
use test_support::fs as test_fs;

/// Stage one manifest in a fresh temporary workspace.
///
/// The query entry point reads from a workspace path, and it refuses to leave
/// the manifest's own directory, so an inline manifest still needs a directory
/// to itself. Staging one also keeps the rows independent: nothing a row writes
/// can be seen by another.
///
/// # Errors
///
/// Returns an error when the temporary directory cannot be created or when the
/// manifest cannot be written.
fn staged(manifest_yaml: &str) -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    let dir = tempfile::tempdir().context("create a workspace for a query load")?;
    let path = dir.path().join("Netsukefile");
    test_fs::write(&path, manifest_yaml)?;
    Ok((dir, path))
}

/// Return the three fixtures every query row loads.
///
/// The three cover the outcomes the entry point can produce: a manifest that
/// loads, a manifest that fails YAML parsing, and a manifest whose rendering
/// differs by surface. The `glob_files/` directory the fourth public fixture
/// reaches through `glob()` is absent by design — under the query surface that
/// helper is a refusal, so a fixture calling it would characterize the refusal
/// rather than the entry point.
fn cases() -> [(&'static str, &'static str); 3] {
    [
        (
            "pure",
            concat!(
                "# Nothing here reads or acts on the host, so the query surface ",
                "admits it.\n",
                "netsuke_version: \"1.0.0\"\n",
                "targets:\n",
                "  - name: pure\n",
                "    description: \"joined {{ ['a', 'b'] | join('-') }}\"\n",
                "    command: \"echo pure\"\n",
            ),
        ),
        ("malformed_yaml", "targets: [{name: broken\n"),
        (
            "boolean_interpolation",
            concat!(
                "# `{{ true }}` is a Jinja literal, so `Display for Value` decides ",
                "its text.\n",
                "# The query registration does not install the legacy Boolean ",
                "formatter the\n",
                "# build registration does, so this fixture is the observable lever ",
                "separating\n",
                "# the two surfaces at the manifest level.\n",
                "netsuke_version: \"1.0.0\"\n",
                "targets:\n",
                "  - name: booleans\n",
                "    description: \"literals: {{ true }}/{{ false }}\"\n",
                "    vars:\n",
                "      enabled: \"{{ true }}\"\n",
                "      disabled: \"{{ false }}\"\n",
                "    command: \"echo {{ enabled }} {{ disabled }}\"\n",
            ),
        ),
    ]
}

/// Load one fixture through the query entry point, keeping its workspace alive.
///
/// The `error` variant is rendered with `{:#}` so the snapshot records the
/// whole `anyhow` chain rather than only its outermost context. A refactor that
/// starts wrapping or dropping a context changes that text, which is exactly
/// what the baseline exists to catch.
fn outcome(label: &str, path: &std::path::Path, limits: ManifestBudgetLimits) -> (String, String) {
    let result = manifest::query::from_path_for_manifest_query_with_limits(path, limits, None);
    let rendered = match &result {
        Ok(manifest) => match serde_json_canonicalizer::to_string(manifest) {
            Ok(canonical) => match serde_json::from_str::<serde_json::Value>(&canonical) {
                Ok(value) => serde_json::to_string_pretty(&value)
                    .unwrap_or_else(|error| format!("<not re-indentable: {error}>")),
                Err(error) => format!("<canonical text not readable: {error}>"),
            },
            Err(error) => format!("<manifest not serializable: {error}>"),
        },
        Err(error) => format!("{error:#}"),
    };
    (label.to_owned(), rendered)
}

/// Snapshot what the crate-internal query entry point produces.
///
/// The budget is left at its default so the rows show the entry point's
/// ordinary behaviour. Exhaustion is a separate row below, which is what keeps
/// this one from recording a limit that is a choice of this test rather than
/// behaviour of the loader.
#[rstest]
fn from_path_with_limits_row() -> Result<()> {
    let mut blocks = Vec::new();
    for (label, text) in cases() {
        let (_dir, path) = staged(text)?;
        let (label, rendered) = outcome(label, &path, ManifestBudgetLimits::default());
        blocks.push(format!("=== {label} ===\n{rendered}"));
    }
    let document = blocks.join("\n\n");
    stable_snapshot_settings("manifest_load_options").bind(|| {
        insta::assert_snapshot!("from_path_with_limits", document);
    });
    Ok(())
}

/// Snapshot the query entry point's budget-exhaustion path.
///
/// Exhaustion is charged against the source bytes first, so a limit below the
/// manifest's own text fails ahead of YAML parsing and rendering. Keeping this
/// as its own row is what lets [`from_path_with_limits_row`] leave the default
/// budget in place: a limit that a test chooses would otherwise be entangled
/// with the entry point's ordinary behaviour in one snapshot, and a reviewer
/// could not tell which change moved it.
///
/// The limit is a constant rather than a computed value, so the diagnostic is
/// repeatable.
#[rstest]
fn budget_exhaustion_row() -> Result<()> {
    let limits = ManifestBudgetLimits {
        source_bytes: 8,
        ..ManifestBudgetLimits::default()
    };
    let (_dir, path) = staged(cases()[0].1)?;
    let (label, rendered) = outcome("pure", &path, limits);
    let document = format!("=== {label} ===\n{rendered}");
    stable_snapshot_settings("manifest_load_options").bind(|| {
        insta::assert_snapshot!("from_path_with_limits_exhausted_budget", document);
    });
    Ok(())
}
