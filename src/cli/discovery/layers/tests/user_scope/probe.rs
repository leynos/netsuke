//! Presence-probe tests for the user-scope fail-closed rule.
//!
//! `OrthoConfig` reports a candidate it could not resolve exactly as it reports
//! one that is not installed, so discovery takes a second look at the directory
//! entry. That look is a seam, and these tests drive it directly: an entry the
//! loader skipped, a link whose target is missing, and an inspection that
//! failed must all stop discovery, while an absent entry must still fall
//! through to the system scope.
//!
//! The inspection failure is the one answer a real directory cannot produce on
//! demand here: the loader opens the same parent first, so it reports that
//! failure before the probe runs. The seam is what keeps the branch testable.
//! The absent and dangling-link answers are also pinned against real
//! directories by the cases in the sibling modules.

use super::{Case, HomeArrangement};
use crate::cli::discovery::discovery_env_source;
use crate::cli::discovery::layers::ProjectIdentity;
use crate::cli::discovery::layers::user_candidate::{
    AutomaticLayers, CandidateEntry, CandidateFailure, CandidateProbe, UnloadableCandidate,
    UserScopeTrace, automatic_base_layers_with_probe,
};
use crate::cli::discovery::paths::FsPathNormalizer;
use crate::cli::test_support::TestEnv;
use crate::localization::{LocalizedMessage, keys};
use anyhow::{Context, Result, ensure};
use ortho_config::{OrthoError, OrthoResult};
use rstest::rstest;
use std::cell::RefCell;
use std::error::Error as _;
use std::io;
use std::path::{Path, PathBuf};

/// How a scripted probe answers every candidate it is asked about.
#[derive(Clone, Copy, Debug)]
pub(super) enum Answer {
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
pub(super) struct ScriptedProbe {
    answer: Answer,
    probed: RefCell<Vec<PathBuf>>,
}

impl ScriptedProbe {
    /// Build a probe that answers every candidate with `answer`.
    pub(super) const fn new(answer: Answer) -> Self {
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
fn layers_with_probe(probe: &dyn CandidateProbe, env: &TestEnv) -> OrthoResult<AutomaticLayers> {
    automatic_base_layers_with_probe(
        probe,
        None,
        discovery_env_source(env),
        ProjectIdentity::new(&FsPathNormalizer, None),
    )
}

/// Return the closed failure payload carried by a rejection.
fn payload_of(error: &OrthoError) -> Option<&UnloadableCandidate> {
    let OrthoError::File { source, .. } = error else {
        return None;
    };
    let io_error = source.downcast_ref::<io::Error>()?;
    io_error.get_ref()?.downcast_ref::<UnloadableCandidate>()
}

/// Return the closed candidate-failure reason carried by a rejection.
fn reason_of(error: &OrthoError) -> Option<CandidateFailure> {
    payload_of(error).map(UnloadableCandidate::reason)
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

    let reason = reason_of(&error);
    ensure!(
        reason == Some(CandidateFailure::PresentWithoutLayer),
        "the rejection must report a present entry that yielded no layer, got {reason:?}: {error}"
    );
    ensure!(
        !probe.probed().is_empty(),
        "discovery must consult the presence probe rather than the loader alone"
    );
    Ok(())
}

/// A dangling link the loader reported as absent fails closed.
///
/// The loader answers `Ok(None)` for a link whose target is missing inside the
/// probed directory, exactly as it does for a file that is not installed. This
/// is the branch that turns that answer into a rejection, so the system scope
/// the user meant to replace cannot supply the policy for it.
#[rstest]
fn dangling_link_entry_stops_discovery() -> Result<()> {
    let case = Case::new()?;
    let probe = ScriptedProbe::new(Answer::Entry(CandidateEntry::Symlink));
    let environment = case.environment(HomeArrangement::EmptyXdgConfigHome);

    let error = layers_with_probe(&probe, &environment)
        .err()
        .context("a dangling link must fail discovery rather than fall through")?;

    ensure!(
        matches!(error.as_ref(), OrthoError::File { .. }),
        "the rejection must be a file error, got {error:?}"
    );
    let reason = reason_of(&error);
    ensure!(
        reason == Some(CandidateFailure::DanglingLink),
        "the rejection must report a dangling link, got {reason:?}: {error}"
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

    let reason = reason_of(&error);
    ensure!(
        reason == Some(CandidateFailure::ProbeFailed),
        "the rejection must report a failed probe, got {reason:?}: {error}"
    );
    let detail = error
        .source()
        .context("the probe failure should retain its cause")?;
    let io_error = detail
        .downcast_ref::<io::Error>()
        .context("the retained cause should be the probe's I/O error")?;
    ensure!(
        io_error.kind() == io::ErrorKind::NotADirectory,
        "the probe's error kind must survive the rejection, got {:?}",
        io_error.kind()
    );

    // The detail is the source, so it is only shown if the message renders it.
    // Requiring both halves keeps a future change from dropping either the
    // catalogued reason or the operator's diagnostic detail.
    let rendered = payload_of(&error)
        .context("the rejection should carry its candidate-failure payload")?
        .to_string();
    let description = LocalizedMessage::new(keys::CONFIG_DISCOVERY_INSPECTION_FAILED).to_string();
    ensure!(
        rendered.contains(&description),
        "the probe failure should be explained by the catalogued reason, got {rendered:?}"
    );
    ensure!(
        rendered.contains("scripted probe failure"),
        "the probe failure should keep the I/O detail alongside the reason, got {rendered:?}"
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

    let found = layers_with_probe(&probe, &environment)
        .context("an absent user candidate must not fail discovery")?;

    // Only a chain rooted at the project file is untrusted, and no project key
    // is supplied here, so the operator-scope system file is the trusted root.
    ensure!(
        found.is_trusted_root,
        "a system-scope chain must report a trusted root"
    );
    ensure!(
        found.user_scope == UserScopeTrace::Fallback,
        "an absent user candidate must be reported as a fallback, got {:?}",
        found.user_scope
    );
    ensure!(
        !probe.probed().is_empty(),
        "discovery must consult the presence probe before falling through"
    );
    let expected = super::canonical(&case.system_config())?;
    let paths: Vec<String> = found
        .layers
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
