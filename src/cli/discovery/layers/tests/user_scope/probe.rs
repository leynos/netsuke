//! Presence-probe tests for the user-scope fail-closed rule.
//!
//! `OrthoConfig` reports a candidate it could not resolve exactly as it reports
//! one that is not installed, so discovery takes a second look at the directory
//! entry. That look is a seam, and these tests drive it directly: the two
//! answers a real filesystem cannot produce on demand — an entry the loader
//! skipped, and an inspection that failed — must stop discovery, while an
//! absent entry must still fall through to the system scope.

use super::{Case, HomeArrangement};
use crate::cli::discovery::discovery_env_source;
use crate::cli::discovery::layers::user_candidate::{
    CandidateEntry, CandidateFailure, CandidateProbe, UnloadableCandidate,
    automatic_base_layers_with_probe,
};
use crate::cli::test_support::TestEnv;
use anyhow::{Context, Result, ensure};
use ortho_config::{MergeLayer, OrthoError, OrthoResult};
use rstest::rstest;
use std::cell::RefCell;
use std::error::Error as _;
use std::io;
use std::path::{Path, PathBuf};

/// How a scripted probe answers every candidate it is asked about.
#[derive(Clone, Copy, Debug)]
enum Answer {
    /// Report this entry state.
    Entry(CandidateEntry),
    /// Fail with this I/O error kind.
    Failed(io::ErrorKind),
}

/// A probe that answers every candidate identically and records the paths.
///
/// Recording matters as much as the answer: a rejection that never consulted
/// the probe would still fail, so the assertions require the probe to have been
/// asked.
struct ScriptedProbe {
    answer: Answer,
    probed: RefCell<Vec<PathBuf>>,
}

impl ScriptedProbe {
    /// Build a probe that answers every candidate with `answer`.
    const fn new(answer: Answer) -> Self {
        Self {
            answer,
            probed: RefCell::new(Vec::new()),
        }
    }

    /// Return the candidate paths the probe was asked about, in order.
    fn probed(&self) -> Vec<PathBuf> {
        self.probed.borrow().clone()
    }
}

impl CandidateProbe for ScriptedProbe {
    fn probe(&self, path: &Path) -> io::Result<CandidateEntry> {
        self.probed.borrow_mut().push(path.to_path_buf());
        match self.answer {
            Answer::Entry(entry) => Ok(entry),
            Answer::Failed(kind) => Err(io::Error::new(kind, "scripted probe failure")),
        }
    }
}

/// Run the automatic discovery pass with `probe` against `env`.
fn layers_with_probe(
    probe: &dyn CandidateProbe,
    env: &TestEnv,
) -> OrthoResult<(Vec<MergeLayer<'static>>, bool)> {
    automatic_base_layers_with_probe(probe, None, discovery_env_source(env), None)
}

/// Return the closed candidate-failure reason carried by a rejection.
fn reason_of(error: &OrthoError) -> Option<CandidateFailure> {
    let OrthoError::File { source, .. } = error else {
        return None;
    };
    let io_error = source.downcast_ref::<io::Error>()?;
    io_error
        .get_ref()?
        .downcast_ref::<UnloadableCandidate>()
        .map(UnloadableCandidate::reason)
}

/// An entry the loader produced no layer for stops discovery.
///
/// This is the race the fail-closed rule exists for: the loader saw no file
/// where the probe sees one. Falling through would hand policy to the system
/// scope the user overrode, so the pass must fail instead, and it must be the
/// probe that noticed.
#[rstest]
fn present_entry_without_a_layer_stops_discovery() -> Result<()> {
    let case = Case::new()?;
    let probe = ScriptedProbe::new(Answer::Entry(CandidateEntry::Present));
    let environment = case.environment(HomeArrangement::EmptyXdgConfigHome);

    let error = layers_with_probe(&probe, &environment)
        .err()
        .context("a present candidate without a layer must fail discovery")?;

    assert_eq!(
        reason_of(&error),
        Some(CandidateFailure::PresentWithoutLayer),
        "the rejection must report a present entry that yielded no layer: {error}"
    );
    ensure!(
        !probe.probed().is_empty(),
        "discovery must consult the presence probe rather than the loader alone"
    );
    Ok(())
}

/// A failed presence probe stops discovery and keeps its detail.
///
/// An inspection error is not absence: treating it as one would silently
/// downgrade a user whose configuration could not be examined, and would lose
/// the error-kind diagnostic the operator needs.
#[rstest]
fn failed_probe_stops_discovery_and_keeps_its_detail() -> Result<()> {
    let case = Case::new()?;
    let probe = ScriptedProbe::new(Answer::Failed(io::ErrorKind::NotADirectory));
    let environment = case.environment(HomeArrangement::EmptyXdgConfigHome);

    let error = layers_with_probe(&probe, &environment)
        .err()
        .context("a failed presence probe must fail discovery")?;

    assert_eq!(
        reason_of(&error),
        Some(CandidateFailure::ProbeFailed),
        "the rejection must report a failed probe: {error}"
    );
    let detail = error
        .source()
        .context("the probe failure should retain its cause")?;
    let io_error = detail
        .downcast_ref::<io::Error>()
        .context("the retained cause should be the probe's I/O error")?;
    assert_eq!(
        io_error.kind(),
        io::ErrorKind::NotADirectory,
        "the probe's error kind must survive the rejection"
    );
    Ok(())
}

/// An absent entry is still skipped, so the system scope keeps working.
///
/// The seam must not turn "nothing installed" into a failure. The dependency's
/// own scan produces the layer here, which is also what proves the pass reached
/// it rather than inventing a layer of its own.
#[rstest]
fn absent_entry_falls_through_to_the_system_scope() -> Result<()> {
    let case = Case::new()?;
    let probe = ScriptedProbe::new(Answer::Entry(CandidateEntry::Absent));
    let environment = case.environment(HomeArrangement::EmptyXdgConfigHome);

    let (layers, is_trusted_root) = layers_with_probe(&probe, &environment)
        .context("an absent user candidate must not fail discovery")?;

    // Only a chain rooted at the project file is untrusted, and no project key
    // is supplied here, so the operator-scope system file is the trusted root.
    ensure!(
        is_trusted_root,
        "a system-scope chain must report a trusted root"
    );
    ensure!(
        !probe.probed().is_empty(),
        "discovery must consult the presence probe before falling through"
    );
    let expected = super::canonical(&case.system_config())?;
    let paths: Vec<String> = layers
        .iter()
        .filter_map(|layer| layer.path())
        .map(|path| super::canonical(Path::new(path.as_str())).unwrap_or_else(|_| path.to_string()))
        .collect();
    ensure!(
        paths == [expected],
        "the system file should be the only layer, got {paths:?}"
    );
    Ok(())
}
