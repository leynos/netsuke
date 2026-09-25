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
//! the span and the failure event are held to an exact field set, and the
//! command, the workspace root, the searched directory, and the pinned
//! extension list are each asserted absent from every field of either.
//!
//! This file holds the workspace fixture and the label vocabulary, which the
//! cases share — including the two that hold the domain's error taxonomy to
//! the vocabulary, one from each side of the boundary. The cases that read a
//! counter series live in [`outcome_series`], and the cases that read a span
//! and its event in [`tracing_capture`]; the split keeps each file to one
//! concern.

use std::{ffi::OsString, num::NonZeroUsize, sync::Arc};

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;
use tempfile::TempDir;

use super::{
    WhichConfig, WhichResolver,
    lookup::WorkspaceSkipList,
    options::{CwdMode, WhichOptions},
    resolve_error::{ResolveError, ResolveErrorCategory},
    telemetry::{
        CATEGORY_ARGS, CATEGORY_CANONICALIZE, CATEGORY_CANONICALIZE_NON_UTF8,
        CATEGORY_CWD_NON_UTF8, CATEGORY_CWD_RESOLVE, CATEGORY_DIRECT_NOT_FOUND,
        CATEGORY_IS_EXECUTABLE, CATEGORY_NOT_FOUND, CATEGORY_WALKDIR, CATEGORY_WORKSPACE_NON_UTF8,
        RESOLVE_ERROR_CATEGORY_VALUES, WHICH_CACHE_OUTCOME_VALUES, WHICH_CWD_MODE_VALUES,
        WHICH_RESOLUTION_OUTCOME_VALUES, category_label, cwd_mode_label,
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

    /// Build a resolver whose PATH and PATHEXT are exactly the overrides given.
    ///
    /// Every case passes an explicit `PATH`, so no fixture's outcome depends
    /// on the ambient environment. `PATHEXT` needs the same treatment for a
    /// different reason: on Windows the fixture executable is written as
    /// `<command>.cmd`, and a host whose `PATHEXT` omits `.CMD` would not
    /// consider it executable, so the hit cases would fail on a correct
    /// resolver. The override is inert off Windows — the snapshot records no
    /// extension list there and nothing consults one — so pinning it
    /// unconditionally costs nothing and keeps the two platforms on one
    /// fixture.
    ///
    /// Arrangement can fail, so this is fallible rather than asserting: the
    /// cache capacity is non-zero by construction of the literal, and a caller
    /// propagates the error instead of unwrapping a value the type system has
    /// not established.
    fn resolver(&self, path: Option<OsString>) -> Result<WhichResolver> {
        let cache_capacity =
            NonZeroUsize::new(8).context("the fixture cache capacity literal must be non-zero")?;
        Ok(WhichResolver::new(
            WhichConfig::new(
                Some(Arc::new(self.root.clone())),
                path,
                WorkspaceSkipList::default(),
                cache_capacity,
            )
            .with_pathext_override(Some(OsString::from(FIXTURE_PATHEXT))),
        ))
    }
}

/// The `PATHEXT` the fixture pins, so a `.cmd` executable resolves on Windows.
///
/// Named rather than inlined because the value is a claim about the fixture
/// executable's filename, and the two have to agree: `tool_filename` appends
/// `.cmd` on Windows, and the extension has to be in this list for the
/// resolver to accept it. `.EXE` leads the list as the host default does.
const FIXTURE_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

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

/// One instance of every `ResolveError` variant, in variant order.
///
/// Built through the real constructors where one exists — `args` and the two
/// free functions — so the fixture cannot describe an error the resolver is
/// unable to produce. The `walkdir` case has no constructor to use, so it is
/// reached through a real failing walk instead.
///
/// Fallible for that last reason: a helper that arranges state is not a test,
/// so it returns `Result` and a caller propagates rather than unwrapping.
fn every_error_variant() -> Result<[ResolveError; 10]> {
    let path = Utf8PathBuf::from("/workspace/tool");
    let io_error = || std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
    Ok([
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
    ])
}

/// Every error variant reports a domain category, and each is reachable.
///
/// The property this used to assert — that `category()` returns a declared
/// telemetry label — is now two properties, because the two are no longer the
/// same thing. This is the domain half: `category()` returns a
/// [`ResolveErrorCategory`], the taxonomy is total over the error type, and
/// every variant reports a category of its own. The spelling half lives in
/// [`every_domain_category_maps_to_a_declared_telemetry_label`], and the two
/// together are what the single assertion used to cover.
///
/// The two length equalities are the load-bearing part: a collision fails the
/// first, and a taxonomy the vocabulary has no room for fails the second.
#[test]
fn every_resolve_error_variant_reports_a_domain_category() -> Result<()> {
    let errors = every_error_variant()?;
    let observed: Vec<ResolveErrorCategory> = errors.iter().map(ResolveError::category).collect();

    let mut distinct = observed.clone();
    distinct.sort_unstable();
    distinct.dedup();
    // `ensure!` rather than `assert!`: a `Result`-returning test reports a
    // failure by returning it, because the workspace denies
    // `clippy::panic_in_result_fn`.
    ensure!(
        distinct.len() == observed.len(),
        "one category per variant, no more and no fewer: {observed:?}"
    );
    ensure!(
        distinct.len() == RESOLVE_ERROR_CATEGORY_VALUES.len(),
        "every category must have a declared label: {distinct:?}"
    );
    Ok(())
}

/// Every domain category maps to exactly one declared telemetry label.
///
/// The boundary half of the property above. It pins the mapping in both
/// directions: the mapped labels are exactly the declared vocabulary, with no
/// duplicates and nothing missing, and the spellings are pinned as a whole so
/// a rename that keeps the set closed cannot pass.
///
/// The mapping is also asserted to be total over the taxonomy — every
/// [`ResolveErrorCategory`] is reachable from a real [`ResolveError`] *and*
/// spells on the wire — which is what makes the count meaningful.
///
/// Note what this does *not* read: the labels come from the boundary's mapping,
/// so nothing here observes what the domain spells a category. That coupling is
/// stated separately, in [`the_domain_spells_every_category_as_its_label`].
#[test]
fn every_domain_category_maps_to_a_declared_telemetry_label() -> Result<()> {
    let errors = every_error_variant()?;
    let mapped: Vec<&str> = errors
        .iter()
        .map(|error| category_label(error.category()))
        .collect();

    ensure!(
        mapped
            .iter()
            .all(|label| RESOLVE_ERROR_CATEGORY_VALUES.contains(label)),
        "every category must map to a declared label: {mapped:?}"
    );
    ensure!(
        mapped.len() == RESOLVE_ERROR_CATEGORY_VALUES.len(),
        "one label per category, no more and no fewer: {mapped:?}"
    );
    ensure!(
        RESOLVE_ERROR_CATEGORY_VALUES
            .iter()
            .all(|declared| mapped.contains(declared)),
        "a declared label is unreachable: {mapped:?}"
    );
    ensure!(
        mapped
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
        "the label set must keep its spellings: {mapped:?}"
    );
    Ok(())
}

/// The domain spells each category exactly as the boundary labels it.
///
/// The domain's own spellings are only asserted here. Neither property above
/// compares them with the labels: the boundary one maps through
/// `category_label`, so a category respelled in the domain alone would leave it
/// holding the same words on the wire and passing. What would notice such a
/// rename is `Display`, which renders the domain's spelling — but only where a
/// case happens to assert the rendered text, which is not an assertion of the
/// coupling.
///
/// So the coupling is stated outright, one variant at a time, by reaching the
/// domain's spelling the only way a consumer can: through the error that
/// reports it.
#[test]
fn the_domain_spells_every_category_as_its_label() -> Result<()> {
    let errors = every_error_variant()?;
    for error in &errors {
        let category = error.category();
        ensure!(
            category.label() == category_label(category),
            "the domain must spell {category:?} as the boundary labels it: \
             domain {:?}, label {:?}",
            category.label(),
            category_label(category)
        );
    }
    // Reached through `Display` as well, because that is the path a reader sees
    // and the one a rename would otherwise only reach by accident.
    for error in &errors {
        if let ResolveError::IsExecutable { .. } = error {
            // This variant renders its source rather than its category, so it
            // is the one place the two legitimately differ.
            continue;
        }
        ensure!(
            error.to_string() == error.category().label(),
            "the rendered error must be the domain's spelling: {error} vs {:?}",
            error.category().label()
        );
    }
    Ok(())
}
