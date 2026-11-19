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
    FileLayerChain, MergeLayer, OrthoError, OrthoResult, SharedEnvSource, load_config_file_as_chain,
};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::debug;

use super::super::diagnostics::path_hash;
use super::super::paths::{PathNormalizer, SanitisedUserBases, user_scope_candidates};
use super::project_key::{CandidateIdentity, ProjectKey};
use super::{chain_layers, config_discovery, first_automatic_chain};
use crate::localization::{LocalizedMessage, keys};

pub(super) mod probe;

pub(super) use probe::{CandidateEntry, CandidateProbe, FsCandidateProbe};

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
    /// No user location supplied a trusted chain, so the dependency scan ran.
    ///
    /// A candidate that is the project configuration is project scope rather
    /// than user scope, so it reaches this branch: the dependency scan composes
    /// the system layers and appends the project chain.
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
/// as documented. A candidate that is the project configuration is not a user
/// file, so the scan also runs for it: adopting it would skip the system layers
/// and drop the restrictions they set. An adopted user chain is rooted in the
/// user's own file rather than in a project root, so it reports a trusted root.
///
/// `normalizer` is the same resolution seam that classified `project_key`, so
/// the key and every candidate are compared through one view of the filesystem.
///
/// # Errors
///
/// Returns an error when a present user candidate cannot be loaded, or when the
/// dependency's scan reports a required or optional failure with no chain.
pub(super) fn automatic_base_layers(
    directory: Option<&Path>,
    env_source: SharedEnvSource,
    normalizer: &impl PathNormalizer,
    project_key: Option<&ProjectKey>,
) -> OrthoResult<AutomaticLayers> {
    automatic_base_layers_with_probe(
        &FsCandidateProbe,
        directory,
        env_source,
        normalizer,
        project_key,
    )
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
    normalizer: &impl PathNormalizer,
    project_key: Option<&ProjectKey>,
) -> OrthoResult<AutomaticLayers> {
    let user_candidates = user_scope_candidates(env_source.as_ref());
    if let Some(found) = user_scope_base_layers(probe, &user_candidates, normalizer, project_key)? {
        return Ok(found);
    }
    // The dependency filters only empty base values, so its scan would resolve a
    // relative one against the working directory. Hand it the same view the
    // user pass used, where such a value reads as unset, and neither path can
    // load configuration from wherever the tool happened to run.
    let fallback_env: SharedEnvSource = Arc::new(SanitisedUserBases::new(env_source));
    let discovery = config_discovery(directory.map(PathBuf::from).as_ref(), fallback_env);
    let (layers, is_trusted_root) =
        first_automatic_chain(&discovery, &user_candidates, normalizer, project_key)?;
    Ok(AutomaticLayers {
        layers,
        is_trusted_root,
        user_scope: UserScopeTrace::Fallback,
    })
}

/// Load the first trusted user-scope configuration file, if one exists.
///
/// A candidate that is the project configuration is refused rather than
/// adopted. The project pass already loads it, with the project quarantine its
/// provenance earns, and adopting it here would return before the system layers
/// load: the `fetch_default_deny` and `env_block_var` restrictions a system file
/// sets would stop applying to a file the project itself controls. Refusing it
/// is decisive — the dependency scan runs instead of a lower-priority candidate
/// being tried — so a project file cannot silently promote a different user
/// file over the system layers. A candidate whose identity cannot be established
/// is refused for the same reason: it cannot be ruled out as the project file.
///
/// The adoption decision travels back with the layers for replay; nothing is
/// logged here, because the tracing filter is not yet raised.
///
/// # Errors
///
/// Returns an error when a candidate exists but cannot be read, parsed, or
/// resolved through its `extends` chain. Falling through to a later candidate
/// would silently substitute another scope, letting a broken user file re-enable
/// the system policy it was meant to override.
fn user_scope_base_layers(
    probe: &dyn CandidateProbe,
    user_candidates: &[PathBuf],
    normalizer: &impl PathNormalizer,
    project_key: Option<&ProjectKey>,
) -> OrthoResult<Option<AutomaticLayers>> {
    for candidate in user_candidates {
        let Some(chain) = load_user_candidate(probe, candidate)? else {
            continue;
        };
        if !candidate_is_trusted(candidate, normalizer, project_key) {
            return Ok(None);
        }
        return Ok(Some(AutomaticLayers {
            layers: chain_layers(chain),
            // Adoption requires a trusted root, so the adopted chain always
            // reports one; the field still distinguishes the fallback's root.
            is_trusted_root: true,
            user_scope: UserScopeTrace::Adopted {
                path_hash: path_hash(candidate),
            },
        }));
    }
    Ok(None)
}

/// Report whether a candidate is a user file rather than the project configuration.
///
/// The project configuration is not a user file, because a project can influence
/// it wherever it is read from. A user candidate is normally a home or XDG
/// location, but the same path can also be the project file — `netsuke` run from
/// `$HOME` makes `$HOME/.netsuke.toml` both — so the user pass applies the same
/// identity test the dependency scan applies. A false answer both withholds
/// operator authority and prevents adoption, because a project-scoped file must
/// not displace the system layers.
///
/// Trust is granted only to a candidate that provably names another file. An
/// ambiguous key or an unresolvable candidate cannot rule the project file out,
/// so both fail closed rather than fall back to a literal miss; a literal
/// spelling that identifies the project file is the one resolution failure that
/// still counts as a match.
pub(super) fn candidate_is_trusted(
    candidate: &Path,
    normalizer: &impl PathNormalizer,
    project_key: Option<&ProjectKey>,
) -> bool {
    project_key.is_none_or(|key| {
        !key.is_ambiguous()
            && key.classify_candidate(normalizer, candidate) == CandidateIdentity::Other
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
        let description = self.reason.description();
        match &self.detail {
            // Printing the detail alone would replace the localised reason
            // rather than add to it, leaving the probe branch as the one
            // rejection an operator sees without a catalogued explanation.
            Some(detail) => write!(f, "{description}: {detail}"),
            None => write!(f, "{description}"),
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
