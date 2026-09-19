//! Tests for the bounded `which` resolver telemetry.
//!
//! Every case drives the real [`WhichResolver`] rather than the recording
//! helpers, so the series these tests assert are the ones a manifest produces.
//! A local `DebuggingRecorder` captures the counters without touching the
//! global recorder, and the tracing cases install a temporary subscriber so
//! the span and the failure event can be inspected field by field.
//!
//! Two properties are pinned here. The `cwd_mode` label distinguishes
//! resolutions that are otherwise identical — the same tool, the same
//! directory, the same outcome — which is exactly what the counters could not
//! do before. And nothing beyond the closed vocabularies leaves the process:
//! the command, the workspace root, and the matched path are asserted absent
//! from every captured event and every captured span field.
//!
//! This file holds the workspace fixture and the label vocabulary, which the
//! cases share. The cases that read a counter series live in
//! [`outcome_series`], and the cases that read a span and its event in
//! [`tracing_capture`]; the split keeps each file to one concern.

use std::{ffi::OsString, num::NonZeroUsize, sync::Arc};

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;
use tempfile::TempDir;

use super::{
    WhichConfig, WhichResolver,
    lookup::WorkspaceSkipList,
    options::{CwdMode, WhichOptions},
    resolve_error::ResolveError,
    telemetry::{
        CATEGORY_ARGS, CATEGORY_CANONICALIZE, CATEGORY_CANONICALIZE_NON_UTF8,
        CATEGORY_CWD_NON_UTF8, CATEGORY_CWD_RESOLVE, CATEGORY_DIRECT_NOT_FOUND,
        CATEGORY_IS_EXECUTABLE, CATEGORY_NOT_FOUND, CATEGORY_WALKDIR, CATEGORY_WORKSPACE_NON_UTF8,
        RESOLVE_ERROR_CATEGORY_VALUES, WHICH_CACHE_OUTCOME_VALUES, WHICH_CWD_MODE_VALUES,
        WHICH_RESOLUTION_OUTCOME_VALUES, cwd_mode_label,
    },
};

/// Name of the resolver span every tracing assertion targets.
const RESOLVER_SPAN: &str = "stdlib.which.resolve";

#[path = "telemetry_tests/outcome_series.rs"]
mod outcome_series;
#[path = "telemetry_tests/tracing_capture.rs"]
mod tracing_capture;

/// A temporary workspace holding the executable fixtures.
struct Workspace {
    /// Guard owning the temporary directory; dropping it removes the fixture.
    _temp: TempDir,
    /// Root of the temporary workspace.
    root: Utf8PathBuf,
    /// Name the resolver is asked for, without a platform extension.
    command: String,
}

impl Workspace {
    /// Create an empty workspace.
    fn new() -> Result<Self> {
        let temp = TempDir::new().context("create temp workspace")?;
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow::anyhow!("temp path should be UTF-8: {}", path.display()))?;
        Ok(Self {
            _temp: temp,
            root,
            command: "telemetry-tool".to_owned(),
        })
    }

    /// Write the fixture executable at the workspace root.
    fn stage_tool(&self) -> Result<()> {
        test_support::write_exec(self.root.as_std_path(), &tool_filename(&self.command))
            .context("write fixture executable")?;
        Ok(())
    }

    /// Build a resolver whose PATH is exactly the supplied override.
    ///
    /// Every case passes an explicit `PATH`, so no fixture's outcome depends
    /// on the ambient environment. Arrangement can fail, so this is fallible
    /// rather than asserting: the cache capacity is non-zero by construction
    /// of the literal, and a caller propagates the error instead of unwrapping
    /// a value the type system has not established.
    fn resolver(&self, path: Option<OsString>) -> Result<WhichResolver> {
        let cache_capacity =
            NonZeroUsize::new(8).context("the fixture cache capacity literal must be non-zero")?;
        Ok(WhichResolver::new(WhichConfig::new(
            Some(Arc::new(self.root.clone())),
            path,
            WorkspaceSkipList::default(),
            cache_capacity,
        )))
    }
}

/// The filename the fixture executable must carry on this platform.
fn tool_filename(command: &str) -> String {
    if cfg!(windows) {
        format!("{command}.cmd")
    } else {
        command.to_owned()
    }
}

/// Join `entries` into a `PATH` value.
fn path_override(entries: &[&Utf8Path]) -> Result<OsString> {
    std::env::join_paths(entries.iter().map(|entry| entry.as_std_path()))
        .context("join PATH entries")
}

/// The default options, with the search domain set to `cwd_mode`.
fn options(cwd_mode: CwdMode) -> WhichOptions {
    WhichOptions {
        cwd_mode,
        ..WhichOptions::default()
    }
}

/// Each search domain maps to its own declared label.
#[rstest]
#[case::auto(CwdMode::Auto, "auto")]
#[case::always(CwdMode::Always, "always")]
#[case::never(CwdMode::Never, "never")]
#[case::workspace_recursive(CwdMode::WorkspaceRecursive, "workspace_recursive")]
fn every_search_domain_maps_to_a_declared_label(
    #[case] mode: CwdMode,
    #[case] expected: &str,
) -> Result<()> {
    ensure!(
        cwd_mode_label(mode) == expected,
        "expected {expected} but labelled {mode:?}"
    );
    ensure!(
        WHICH_CWD_MODE_VALUES.contains(&expected),
        "{expected} must be declared in the closed vocabulary"
    );
    Ok(())
}

/// The label is the telemetry spelling, and the template's is not.
///
/// A manifest writes `workspace-recursive`; the label must not, because a
/// hyphen in a metric name or label is a spelling some exporters rewrite.
/// Pinning the two spellings side by side keeps the mapping deliberate rather
/// than incidental.
#[test]
fn the_recursive_label_is_not_the_template_spelling() -> Result<()> {
    let parsed = CwdMode::parse("workspace-recursive")
        .context("the template spelling must parse to the recursive mode")?;
    ensure!(
        cwd_mode_label(parsed) == "workspace_recursive",
        "the template spelling must not become the label"
    );
    ensure!(
        !WHICH_CWD_MODE_VALUES.contains(&"workspace-recursive"),
        "the hyphenated template spelling is not a telemetry value"
    );
    Ok(())
}

/// Each closed vocabulary declares every value once.
#[test]
fn the_label_vocabularies_are_closed_and_duplicate_free() {
    for (name, values) in [
        ("cwd_mode", &WHICH_CWD_MODE_VALUES[..]),
        ("cache outcome", &WHICH_CACHE_OUTCOME_VALUES[..]),
        ("resolution outcome", &WHICH_RESOLUTION_OUTCOME_VALUES[..]),
        ("category", &RESOLVE_ERROR_CATEGORY_VALUES[..]),
    ] {
        let mut sorted = values.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            values.len(),
            "the {name} vocabulary must declare each value once: {values:?}"
        );
    }
}

/// A `walkdir` error, produced by listing a directory that does not exist.
///
/// `walkdir::Error` has no public constructor, so the fixture is reached
/// through the walk itself.
fn walkdir_error() -> Result<walkdir::Error> {
    walkdir::WalkDir::new("/netsuke/which/telemetry/no/such/directory")
        .into_iter()
        .next()
        .context("a walk always yields at least its root")?
        .err()
        .context("a missing root must fail the walk")
}

/// Every error variant reports a declared category, and each is reachable.
///
/// Length equality is the load-bearing half: it fails both when a variant
/// reports an undeclared value and when the vocabulary declares a value no
/// variant can produce, so the label set cannot drift away from the error
/// type. The final assertion pins the spelling, because a rename that keeps
/// the set closed but changes a label would otherwise pass.
#[test]
fn every_resolve_error_variant_reports_a_declared_category() -> Result<()> {
    let path = Utf8PathBuf::from("/workspace/tool");
    let io_error = || std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
    let errors = [
        ResolveError::NotFound {
            command: "tool".to_owned(),
            dirs: Vec::new(),
            cwd_mode: CwdMode::Never,
        },
        ResolveError::DirectNotFound {
            command: "tool".to_owned(),
            path: path.clone(),
        },
        ResolveError::args("bad option"),
        ResolveError::Canonicalize {
            path: path.clone(),
            source: io_error(),
        },
        ResolveError::IsExecutable {
            path: path.clone(),
            source: io_error(),
        },
        ResolveError::CanonicalizeNonUtf8,
        ResolveError::WorkspaceNonUtf8 {
            command: "tool".to_owned(),
            path: path.to_string(),
        },
        ResolveError::WalkDir {
            source: walkdir_error()?,
        },
        ResolveError::CwdResolve { source: io_error() },
        ResolveError::CwdNonUtf8,
    ];

    let observed: Vec<&str> = errors.iter().map(ResolveError::category).collect();
    // `ensure!` rather than `assert!`: a `Result`-returning test reports a
    // failure by returning it, because the workspace denies
    // `clippy::panic_in_result_fn`.
    ensure!(
        observed
            .iter()
            .all(|category| RESOLVE_ERROR_CATEGORY_VALUES.contains(category)),
        "every category must be declared: {observed:?}"
    );
    ensure!(
        observed.len() == RESOLVE_ERROR_CATEGORY_VALUES.len(),
        "one category per variant, no more and no fewer: {observed:?}"
    );
    ensure!(
        RESOLVE_ERROR_CATEGORY_VALUES
            .iter()
            .all(|declared| observed.contains(declared)),
        "a declared category is unreachable: {observed:?}"
    );
    ensure!(
        observed
            == [
                CATEGORY_NOT_FOUND,
                CATEGORY_DIRECT_NOT_FOUND,
                CATEGORY_ARGS,
                CATEGORY_CANONICALIZE,
                CATEGORY_IS_EXECUTABLE,
                CATEGORY_CANONICALIZE_NON_UTF8,
                CATEGORY_WORKSPACE_NON_UTF8,
                CATEGORY_WALKDIR,
                CATEGORY_CWD_RESOLVE,
                CATEGORY_CWD_NON_UTF8,
            ],
        "the taxonomy must keep its spellings: {observed:?}"
    );
    Ok(())
}
