//! What each public loader entry point produces today.
//!
//! Eleven public entry points load a manifest, and they differ only in how much
//! of the loader's seam they accept: a network policy, an environment reader or
//! a paired environment, resource ceilings, a stdlib configuration, or a stage
//! observer. The refactor that replaces their shared core must leave every one
//! of them producing exactly what it produces now, and the only trustworthy
//! oracle for that claim is the code before the refactor — so these snapshots
//! are recorded against the unchanged loader and re-checked at every later
//! milestone.
//!
//! Comparing a wrapper against the core *after* the refactor would be equal by
//! construction and would prove nothing. That is why the baseline is recorded
//! first and never re-accepted afterwards.
//!
//! # One row, one document
//!
//! A row snapshots one document covering every fixture it may load, rather than
//! one snapshot per fixture. The directory stays small enough to hold in view,
//! a reviewer reads one file to see whether a refactor changed *anything* about
//! an entry point, and the documents are diffable against each other — which is
//! what turns "these wrappers delegate identically" from an assertion into an
//! observation.
//!
//! Each snapshot name is written out rather than derived from the test
//! function. Three pairs of entry points differ only in a suffix (`_and_env`
//! against `_and_environment`, and the `_and_limits` forms of both), so a
//! derived name would read as the same row spelled twice.
//!
//! # Applicability and the clock
//!
//! What each row may load is decided by [`Case::is_loadable`] against the
//! loader's [`Capabilities`], and a row that finds no applicable fixture fails
//! rather than snapshotting an empty document. Without that guard a mistake in
//! the capability table would silently reduce an entry point's characterization
//! to nothing while the snapshot stayed green.
//!
//! Only the entries that accept a [`StdlibConfig`] can pin the clock, so
//! `clock_now` is loaded only there. That is a property of the public surface
//! rather than a choice: a path entry point builds its own configuration from
//! the workspace it opens and has no seam for a caller's clock, so
//! [`path_rows::path_entry_point_cannot_inject_a_clock`] measures that rather
//! than leaving it as an assumption the capability table happens to encode.
//!
//! # Splitting the rows
//!
//! [`path_rows`] holds every row whose entry point opens a workspace, and this
//! module holds the string rows plus the shared harness. The split is the
//! string-versus-path seam, not a line count: a path row stages a fixture
//! before it can load anything, and a string row never does.

mod path_rows;

use anyhow::Result;
use camino::Utf8Path;
use rstest::rstest;

use netsuke::ast::NetsukeManifest;
use netsuke::manifest::{
    EnvAccessPolicy, ManifestLoadStage, from_str, from_str_with_env, from_str_with_env_and_config,
    from_str_with_env_and_policy,
};
use netsuke::stdlib::NetworkPolicy;

use super::fixtures::{
    Capabilities, Case, StageSeam, StagedFixture, cases, fixed_clock_config, injected_environment,
    injected_environment_and_clock, one_var_reader, process_environment, report, settings,
};

/// Return the fixtures a loader with `capabilities` may load.
pub(super) fn applicable(capabilities: Capabilities) -> Vec<Case> {
    cases()
        .iter()
        .copied()
        .filter(|case| case.is_loadable(capabilities))
        .collect()
}

/// Render the document a row snapshots, staging each fixture in turn.
///
/// `load` receives the fixture, the path a path entry point should read, and
/// the buffer an observer should append to: the staged copy lives in a fresh
/// temporary workspace.
///
/// The observer's stages are recorded per fixture, in the block that fixture
/// produced. A shared buffer would misreport every block but the first, because
/// a load that fails early notifies fewer stages than one that succeeds, and
/// the entry point's own line already records where it stopped.
///
/// `seam` states whether the entry point offers an observer at all. It is the
/// caller's claim rather than something this function can infer, so a row that
/// passes `Accepted` while its entry point takes no observer fails to compile,
/// and one that passes `Absent` for an observer-taking entry point would leave
/// its closures unused.
///
/// # Errors
///
/// Returns an error when the capability table admits no fixture — the row would
/// snapshot an empty document and characterize nothing — or when a fixture
/// cannot be staged or read.
pub(super) fn document<F>(
    capabilities: Capabilities,
    seam: StageSeam,
    mut load: F,
) -> Result<String>
where
    F: FnMut(&Case, &Utf8Path, &mut Vec<ManifestLoadStage>) -> Result<NetsukeManifest>,
{
    let applicable = applicable(capabilities);
    anyhow::ensure!(
        !applicable.is_empty(),
        "no fixture is applicable to this entry point; the capability table is wrong"
    );
    let mut blocks = Vec::with_capacity(applicable.len());
    for case in applicable {
        let fixture = StagedFixture::new(&case)?;
        let mut stages = Vec::new();
        let outcome = load(&case, fixture.manifest_path(), &mut stages);
        blocks.push(report(&[(case.label.to_owned(), outcome)], seam, &stages));
    }
    Ok(blocks.join("\n\n"))
}

/// Render the document a row snapshots for a string entry point.
///
/// A string load names the manifest only for diagnostics, so the fixture's own
/// repository-relative path serves as that name. Naming it anything else would
/// suggest the loader had been handed a workspace it does not have.
///
/// No string entry point takes a stage observer, so the rows built on this
/// helper are all [`StageSeam::Absent`].
///
/// # Errors
///
/// Returns an error under the same conditions as [`document`], plus when a
/// fixture cannot be read.
fn document_from_string<F>(capabilities: Capabilities, mut load: F) -> Result<String>
where
    F: FnMut(&str) -> Result<NetsukeManifest>,
{
    document(capabilities, StageSeam::Absent, |case, _path, _stages| {
        load(&case.manifest_text()?)
    })
}

/// Return the policy every policy-taking row passes.
///
/// `NetworkPolicy::default` is HTTPS-only. No applicable fixture reaches the
/// network: `uses_fetch` addresses `http://127.0.0.1:1/`, which the policy
/// refuses at the scheme check before a socket is opened, and only a
/// refusing-surface row loads it. The policy is therefore not what any row
/// characterizes; it is the value an ordinary caller supplies.
pub(super) fn policy() -> NetworkPolicy {
    NetworkPolicy::default()
}

/// Return the permissive access policy the adapters default to.
fn permissive() -> EnvAccessPolicy {
    EnvAccessPolicy::default()
}

/// Snapshot what `from_str` produces.
///
/// `from_str` takes no seam at all: it reads the process environment and the
/// host clock. It is therefore the narrowest row, and the fixtures it may load
/// are exactly the ones no seam can change.
#[rstest]
fn from_str_row() -> Result<()> {
    let document = document_from_string(process_environment(), from_str)?;
    settings().bind(|| insta::assert_snapshot!("from_str", document));
    Ok(())
}

#[rstest]
fn from_str_with_env_row() -> Result<()> {
    let document = document_from_string(injected_environment(), |text| {
        from_str_with_env(text, &one_var_reader())
    })?;
    settings().bind(|| insta::assert_snapshot!("from_str_with_env", document));
    Ok(())
}

#[rstest]
fn from_str_with_env_and_policy_row() -> Result<()> {
    let document = document_from_string(injected_environment(), |text| {
        from_str_with_env_and_policy(text, &one_var_reader(), &permissive())
    })?;
    settings().bind(|| {
        insta::assert_snapshot!("from_str_with_env_and_policy", document);
    });
    Ok(())
}

#[rstest]
fn from_str_with_env_and_config_row() -> Result<()> {
    let document = document_from_string(injected_environment_and_clock(), |text| {
        from_str_with_env_and_config(text, &one_var_reader(), fixed_clock_config()?)
    })?;
    settings().bind(|| {
        insta::assert_snapshot!("from_str_with_env_and_config", document);
    });
    Ok(())
}
