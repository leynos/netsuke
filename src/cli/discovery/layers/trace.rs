//! Bounded discovery decisions retained for a deferred trace replay.
//!
//! Discovery runs before the composition boundary raises the tracing filter, so
//! an event emitted where a decision is made would be dropped. These decisions
//! travel with the discovered layers instead and are replayed once the filter
//! admits them, without repeating discovery or touching the environment.

use super::super::diagnostics::{
    BoundedConfigPath, ProjectLayerDeduplication, debug_optional_config_path_from_fields,
};
use super::user_candidate::UserScopeTrace;

/// Project-scope outcome retained for a later trace replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::cli::discovery) enum ProjectScopeTrace {
    /// The primary discovery scan already yielded the project configuration.
    Included(BoundedConfigPath),
    /// The project configuration was loaded by the second pass.
    Appended {
        /// Project config path loaded by the fallback pass.
        path: BoundedConfigPath,
        /// Bounded de-duplication outcome, when one was recorded.
        deduplication: Option<ProjectLayerDeduplication>,
    },
    /// The second pass found only layers already returned by discovery.
    Deduplicated {
        /// Project config path already returned by the primary scan.
        path: BoundedConfigPath,
        /// Bounded de-duplication outcome for the repeated project layer.
        deduplication: ProjectLayerDeduplication,
    },
}

impl ProjectScopeTrace {
    /// Replay the original project-scope diagnostic from bounded metadata.
    pub(in crate::cli::discovery) fn emit(&self) {
        match self {
            Self::Included(path) => {
                debug_optional_config_path_from_fields(
                    "discovery included project-scope layers",
                    path,
                );
            }
            Self::Appended {
                path,
                deduplication,
            } => {
                if let Some(counts) = deduplication {
                    counts.emit();
                }
                debug_optional_config_path_from_fields("appending project-scope layers", path);
            }
            Self::Deduplicated {
                path,
                deduplication,
            } => {
                deduplication.emit();
                debug_optional_config_path_from_fields(
                    "project-scope layers already discovered",
                    path,
                );
            }
        }
    }
}

/// Decisions the automatic pass retains for a later verbose replay.
///
/// Discovery runs before the composition boundary raises the tracing filter, so
/// an event emitted where the decision is made would be dropped. These bounded
/// decisions travel with the layers instead and are replayed once the filter
/// admits them.
#[derive(Clone, Debug, Default)]
pub(in crate::cli::discovery) struct AutomaticTrace {
    /// Which scope supplied the automatic base chain, when the pass decided.
    user_scope: Option<UserScopeTrace>,
    /// Trail of the project-scope second pass, when one ran.
    project_scope: Option<ProjectScopeTrace>,
}

impl AutomaticTrace {
    /// Combine a user-pass decision with any project-scope outcome.
    pub(in crate::cli::discovery) const fn new(
        user_scope: Option<UserScopeTrace>,
        project_scope: Option<ProjectScopeTrace>,
    ) -> Self {
        Self {
            user_scope,
            project_scope,
        }
    }

    /// Emit both retained decisions without repeating discovery.
    pub(in crate::cli::discovery) fn emit(&self) {
        if let Some(user_scope) = &self.user_scope {
            user_scope.emit();
        }
        if let Some(project_scope) = &self.project_scope {
            project_scope.emit();
        }
    }
}
