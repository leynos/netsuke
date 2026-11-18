//! The Netsuke-owned user-scope discovery pass and its fail-closed reasons.
//!
//! `OrthoConfig` 0.10.0 interleaves `XDG_CONFIG_DIRS` between the user and home
//! candidates, so a system file can hide the user's own configuration. Netsuke
//! therefore walks the user locations itself before handing the dependency's
//! scan the sanitised remainder, and it rejects a candidate that is present but
//! unloadable rather than falling through to a lower scope.
//!
//! [`crate::cli::discovery::layers`] is the only consumer: it calls
//! [`automatic_base_layers`], and discovery telemetry reads the closed
//! [`CandidateFailure`] reason carried by a rejected candidate's error so it can
//! label the fail-closed branch without reading error text.

use ortho_config::{
    EnvSource, FileLayerChain, MergeLayer, OrthoError, OrthoResult, SharedEnvSource,
    load_config_file_as_chain,
};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::debug;

use super::super::diagnostics::path_hash;
use super::super::paths::{
    FsPathNormalizer, SanitisedUserBases, candidate_entry_metadata, user_scope_candidates,
};
use super::project_key::ProjectKey;
use super::{chain_layers, config_discovery, first_automatic_chain};
use crate::localization::{LocalizedMessage, keys};

/// Automatic discovery result, with the user pass's decision retained.
///
/// The decision travels with the layers rather than being logged where it is
/// made: discovery runs before the composition boundary installs the tracing
/// filter, so a `DEBUG` event emitted here would be dropped in a verbose run.
/// [`UserScopeTrace`] replays it once the filter admits it.
pub(super) struct AutomaticLayers {
    /// Base merge layers in automatic-discovery order.
    pub(super) layers: Vec<MergeLayer<'static>>,
    /// Whether the root that supplied the chain may donate operator authority.
    pub(super) is_trusted_root: bool,
    /// Which scope supplied the chain.
    pub(super) user_scope: UserScopeTrace,
}

/// Which scope supplied the automatic base chain.
///
/// Both variants are bounded: the adopted case carries a run-local correlation
/// hash rather than the candidate path, and the fallback case names no path at
/// all because the dependency scan reads several locations and naming one would
/// misreport which file supplied policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::cli::discovery) enum UserScopeTrace {
    /// A user location supplied the chain.
    Adopted {
        /// Run-local correlation hash of the adopted candidate's path.
        path_hash: String,
    },
    /// No user location supplied one, so the dependency scan ran.
    Fallback,
}

impl UserScopeTrace {
    /// Replay the adoption or fallback diagnostic from bounded metadata.
    pub(in crate::cli::discovery) fn emit(&self) {
        match self {
            Self::Adopted { path_hash } => debug!(
                scope = "user",
                skipped_compose_layers = true,
                path_hash = %path_hash,
                "automatic configuration discovery adopted a user candidate"
            ),
            Self::Fallback => debug!(
                scope = "system_or_project",
                skipped_compose_layers = false,
                "automatic configuration discovery used the dependency scan"
            ),
        }
    }
}

/// Load the automatic discovery chain, preferring the user scope.
///
/// The user pass runs first so a valid system file cannot outrank the user's own
/// configuration. The dependency's scan runs only when no user file exists, so
/// system-provided configuration and the project roots it contributes still work
/// as documented. An adopted user chain is rooted in the user's own file rather
/// than in a project root, so it reports a trusted root.
///
/// # Errors
///
/// Returns an error when a present user candidate cannot be loaded, or when the
/// dependency's scan reports a required or optional failure with no chain.
pub(super) fn automatic_base_layers(
    directory: Option<&Path>,
    env_source: SharedEnvSource,
    project_key: Option<&ProjectKey>,
) -> OrthoResult<AutomaticLayers> {
    automatic_base_layers_with_probe(&FsCandidateProbe, directory, env_source, project_key)
}

/// Load the automatic discovery chain, consulting `probe` about user candidates.
///
/// The presence check is a parameter rather than a direct filesystem call so the
/// two rare fail-closed branches — an entry that exists yet yields no layer, and
/// a probe that itself fails — can be driven from a test. Both are races against
/// the loader's own view of the directory, so a real filesystem fixture cannot
/// reproduce them on demand.
///
/// # Errors
///
/// Returns an error when a present user candidate cannot be loaded, or when the
/// dependency's scan reports a required or optional failure with no chain.
pub(super) fn automatic_base_layers_with_probe(
    probe: &dyn CandidateProbe,
    directory: Option<&Path>,
    env_source: SharedEnvSource,
    project_key: Option<&ProjectKey>,
) -> OrthoResult<AutomaticLayers> {
    if let Some(found) = user_scope_base_layers(probe, env_source.as_ref(), project_key)? {
        return Ok(found);
    }
    // The dependency filters only empty base values, so its scan would resolve a
    // relative one against the working directory. Hand it the same view the
    // user pass used, where such a value reads as unset, and neither path can
    // load configuration from wherever the tool happened to run.
    let fallback_env: SharedEnvSource = Arc::new(SanitisedUserBases::new(env_source));
    let discovery = config_discovery(directory.map(PathBuf::from).as_ref(), fallback_env);
    let (layers, is_trusted_root) = first_automatic_chain(&discovery, project_key)?;
    Ok(AutomaticLayers {
        layers,
        is_trusted_root,
        user_scope: UserScopeTrace::Fallback,
    })
}

/// Load the first user-scope configuration file, if one exists.
///
/// The layers are returned with the trust their root warrants, computed exactly
/// as the dependency scan computes it, so adopting a user file cannot quietly
/// upgrade the authority of a path that is also the project configuration. The
/// adoption decision travels back with them for replay; nothing is logged here,
/// because the tracing filter is not yet raised.
///
/// # Errors
///
/// Returns an error when a candidate exists but cannot be read, parsed, or
/// resolved through its `extends` chain. Falling through to a later candidate
/// would silently substitute another scope, letting a broken user file re-enable
/// the system policy it was meant to override.
fn user_scope_base_layers(
    probe: &dyn CandidateProbe,
    env_source: &dyn EnvSource,
    project_key: Option<&ProjectKey>,
) -> OrthoResult<Option<AutomaticLayers>> {
    user_scope_candidates(env_source)
        .into_iter()
        .find_map(|candidate| match load_user_candidate(probe, &candidate) {
            Ok(Some(chain)) => Some(Ok(AutomaticLayers {
                layers: chain_layers(chain),
                is_trusted_root: candidate_is_trusted(&candidate, project_key),
                user_scope: UserScopeTrace::Adopted {
                    path_hash: path_hash(&candidate),
                },
            })),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        })
        .transpose()
}

/// Report whether a candidate that supplied a chain may donate operator authority.
///
/// Operator authority is withheld from the project configuration file, because a
/// project can influence it wherever it is read from. A user candidate is
/// normally a home or XDG location, but the same path can also be the project
/// file — `netsuke` run from `$HOME` makes `$HOME/.netsuke.toml` both — so the
/// user pass applies the same identity test the dependency scan applies rather
/// than assuming the user scope is trusted.
pub(super) fn candidate_is_trusted(candidate: &Path, project_key: Option<&ProjectKey>) -> bool {
    !project_key.is_some_and(|key| {
        key.is_ambiguous() || key.matches_candidate(&FsPathNormalizer, candidate)
    })
}

/// Load one user-scope candidate, distinguishing absence from a broken link.
///
/// `OrthoConfig` resolves a candidate through a capability on its parent
/// directory, so a link whose target does not resolve is reported exactly as a
/// file that is not installed. Probing the entry separates the two, so a
/// dangling `$HOME/.netsuke.toml` cannot hand policy back to system scope.
///
/// # Errors
///
/// Returns the loader error when the candidate exists but cannot be read,
/// parsed, or resolved through its `extends` chain, and an error naming the
/// candidate when the loader reported it as absent although the entry exists.
fn load_user_candidate(
    probe: &dyn CandidateProbe,
    path: &Path,
) -> OrthoResult<Option<FileLayerChain>> {
    if let Some(chain) = load_config_file_as_chain(path)? {
        return Ok(Some(chain));
    }
    match probe.probe(path) {
        Ok(CandidateEntry::Symlink) => {
            Err(unloadable_candidate(path, CandidateFailure::DanglingLink))
        }
        Ok(CandidateEntry::Present) => Err(unloadable_candidate(
            path,
            CandidateFailure::PresentWithoutLayer,
        )),
        Ok(CandidateEntry::Absent) => Ok(None),
        Err(error) => Err(probe_failure(path, error)),
    }
}

/// What a [`CandidateProbe`] found at a user candidate path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CandidateEntry {
    /// Nothing exists at the path.
    Absent,
    /// The entry is a symbolic link whose target did not resolve.
    Symlink,
    /// An entry exists and is not a symbolic link.
    Present,
}

/// Presence probe for one user-scope candidate.
///
/// The loader reports an unreadable candidate exactly as it reports an absent
/// one, so the fail-closed rule needs a second look at the directory entry
/// itself. Keeping that look behind a trait lets a test drive the three
/// answers, and the probe's own failure, without staging a directory that
/// happens to break the loader first.
pub(super) trait CandidateProbe {
    /// Report what the filesystem holds at `path`.
    ///
    /// # Errors
    ///
    /// Returns the inspection error when the entry cannot be examined, for
    /// example because a parent component is not a directory.
    fn probe(&self, path: &Path) -> io::Result<CandidateEntry>;
}

/// The filesystem-backed probe production discovery uses.
struct FsCandidateProbe;

impl CandidateProbe for FsCandidateProbe {
    fn probe(&self, path: &Path) -> io::Result<CandidateEntry> {
        candidate_entry_metadata(path).map(|found| match found {
            Some(metadata) if metadata.file_type().is_symlink() => CandidateEntry::Symlink,
            Some(_) => CandidateEntry::Present,
            None => CandidateEntry::Absent,
        })
    }
}

/// Closed reason a user-scope candidate could not supply configuration.
///
/// The reason travels with the error as a typed payload rather than as part of
/// its message, so the discovery telemetry can report which fail-closed branch
/// fired without reading free text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::cli::discovery) enum CandidateFailure {
    /// The candidate is a symbolic link whose target does not resolve.
    DanglingLink,
    /// The entry exists yet the loader produced no layer for it.
    PresentWithoutLayer,
    /// Inspecting the candidate entry itself failed.
    ProbeFailed,
}

impl CandidateFailure {
    /// Return the localised diagnostic shown to the user for this reason.
    ///
    /// The text is looked up when the error is rendered rather than when it is
    /// built, so a locale chosen after discovery still applies to the message.
    const fn description(self) -> LocalizedMessage {
        LocalizedMessage::new(match self {
            Self::DanglingLink => keys::CONFIG_DISCOVERY_DANGLING_LINK,
            Self::PresentWithoutLayer => keys::CONFIG_DISCOVERY_PRESENT_WITHOUT_LAYER,
            Self::ProbeFailed => keys::CONFIG_DISCOVERY_INSPECTION_FAILED,
        })
    }

    /// Return the closed telemetry label for this reason.
    pub(in crate::cli::discovery) const fn label(self) -> &'static str {
        match self {
            Self::DanglingLink => "dangling_link",
            Self::PresentWithoutLayer => "present_without_layer",
            Self::ProbeFailed => "probe_failed",
        }
    }
}

/// Error payload carrying a candidate failure's closed reason.
///
/// The original probe error, when there is one, is retained as the source so
/// the user-visible message keeps its diagnostic detail.
#[derive(Debug)]
pub(in crate::cli::discovery) struct UnloadableCandidate {
    /// Closed reason the candidate was rejected.
    reason: CandidateFailure,
    /// Original probe error, present only for [`CandidateFailure::ProbeFailed`].
    detail: Option<io::Error>,
}

impl UnloadableCandidate {
    /// Return the closed reason recorded when this candidate failed.
    pub(in crate::cli::discovery) const fn reason(&self) -> CandidateFailure {
        self.reason
    }
}

impl fmt::Display for UnloadableCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.detail {
            Some(detail) => write!(f, "{detail}"),
            None => write!(f, "{}", self.reason.description()),
        }
    }
}

impl std::error::Error for UnloadableCandidate {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.detail
            .as_ref()
            .map(|detail| detail as &(dyn std::error::Error + 'static))
    }
}

/// Build the error for a candidate that exists but cannot supply configuration.
fn unloadable_candidate(path: &Path, reason: CandidateFailure) -> Arc<OrthoError> {
    candidate_failure_error(path, reason, None)
}

/// Build the error for a candidate whose presence probe itself failed.
fn probe_failure(path: &Path, error: io::Error) -> Arc<OrthoError> {
    candidate_failure_error(path, CandidateFailure::ProbeFailed, Some(error))
}

/// Build a file error whose source carries a closed candidate-failure reason.
///
/// The marker is nested inside an [`io::Error`] so any consumer that expects a
/// file error's source to be an I/O error still finds one, while
/// [`CandidateFailure`] stays available to telemetry through a downcast.
pub(in crate::cli::discovery) fn candidate_failure_error(
    path: &Path,
    reason: CandidateFailure,
    detail: Option<io::Error>,
) -> Arc<OrthoError> {
    let kind = detail
        .as_ref()
        .map_or(io::ErrorKind::InvalidInput, io::Error::kind);
    Arc::new(OrthoError::File {
        path: path.to_path_buf(),
        source: Box::new(io::Error::new(kind, UnloadableCandidate { reason, detail })),
    })
}
