//! Bounded metadata for replaying configuration discovery diagnostics.
//!
//! Discovery captures this data while it still owns the selected paths and
//! project-scope result. Startup later replays the original trace events after
//! it enables verbose output, without accessing the environment or filesystem.

use tracing::debug;

use super::diagnostics::{
    BoundedConfigPath, ConfigLoadWarning, debug_config_path_from_fields,
    trace_config_path_variable_from_fields,
};
use super::layers::ProjectScopeTrace;
use super::selector::ConfigPathResolution;

/// Track only the authority category and coarse count of removed grants.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct EnvGrantQuarantineTrace {
    /// Grants removed from an explicitly selected project primary layer.
    project_primary: Option<GrantCountBucket>,
    /// Grants removed from occurrences reached through automatic project discovery.
    project_chain: Option<GrantCountBucket>,
}

impl EnvGrantQuarantineTrace {
    /// Add one occurrence's grant count to its bounded provenance bucket.
    pub(super) fn record(&mut self, project_chain: bool, grant_count: usize) {
        let bucket = if project_chain {
            &mut self.project_chain
        } else {
            &mut self.project_primary
        };
        *bucket = Some(bucket.map_or_else(
            || GrantCountBucket::from_count(grant_count),
            |current| current.add(grant_count),
        ));
    }

    /// Emit one deferred event for each project provenance category present.
    pub(super) fn emit(self) {
        if let Some(grant_count) = self.project_primary {
            tracing::debug!(
                authority = "project_primary",
                grant_count = grant_count.label(),
                "quarantined project environment grants"
            );
        }
        if let Some(grant_count) = self.project_chain {
            tracing::debug!(
                authority = "project_chain",
                grant_count = grant_count.label(),
                "quarantined project environment grants"
            );
        }
    }
}

/// Bound removed-grant telemetry without retaining names or exact counts.
#[derive(Clone, Copy, Debug)]
enum GrantCountBucket {
    /// A valid but empty grant list was removed.
    Zero,
    /// One grant was removed.
    One,
    /// Two or more grants were removed.
    Multiple,
}

impl GrantCountBucket {
    /// Map a grant count into a closed, bounded category.
    const fn from_count(count: usize) -> Self {
        match count {
            0 => Self::Zero,
            1 => Self::One,
            _ => Self::Multiple,
        }
    }

    /// Combine another occurrence's grant count without retaining exact totals.
    const fn add(self, count: usize) -> Self {
        match (self, count) {
            (Self::Multiple, _) | (_, 2..) | (Self::One, 1) => Self::Multiple,
            (Self::Zero, 0) => Self::Zero,
            (Self::Zero, 1) | (Self::One, 0) => Self::One,
        }
    }

    /// Return this bucket's stable telemetry label.
    const fn label(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::One => "one",
            Self::Multiple => "multiple",
        }
    }
}

/// Bounded selector and layer-branch diagnostics retained after discovery.
#[derive(Clone, Debug)]
pub(super) struct DiscoveryTrace {
    /// Bounded trace of selector resolution and its environment reads.
    resolution: ConfigPathTrace,
    /// Bounded trace of the adopted file-layer branch.
    file_layers: FileLayerTrace,
}

impl DiscoveryTrace {
    /// Combine a resolved selector and layer branch without retaining raw paths.
    pub(super) fn new(resolution: &ConfigPathResolution, file_layers: FileLayerTrace) -> Self {
        Self {
            resolution: ConfigPathTrace::from_resolution(resolution),
            file_layers,
        }
    }

    /// Emit all discovery diagnostics from bounded metadata only.
    pub(super) fn emit(&self) {
        self.resolution.emit();
        self.file_layers.emit();
    }
}

/// Bounded diagnostics for selector resolution and its environment reads.
#[derive(Clone, Debug)]
struct ConfigPathTrace {
    /// Configuration selector that resolved the path.
    selector: &'static str,
    /// Bounded resolved configuration path.
    path: BoundedConfigPath,
    /// Environment variables consulted for path resolution, with bounded results.
    environment_lookups: Vec<(&'static str, BoundedConfigPath)>,
}

impl ConfigPathTrace {
    /// Build a trace-only view without retaining raw paths.
    fn from_resolution(resolution: &ConfigPathResolution) -> Self {
        Self {
            selector: resolution.selector,
            path: BoundedConfigPath::from_path(resolution.path.as_deref()),
            environment_lookups: resolution
                .environment_lookups
                .iter()
                .map(|(var_name, path)| (*var_name, BoundedConfigPath::from_path(path.as_deref())))
                .collect(),
        }
    }

    /// Emit selector diagnostics from their bounded representation.
    fn emit(&self) {
        for (var_name, path) in &self.environment_lookups {
            trace_config_path_variable_from_fields(var_name, path);
        }
        debug!(
            selector = self.selector,
            path_hash = self.path.hash.as_deref(),
            path_present = self.path.is_present,
            "resolved config path"
        );
    }
}

/// File-layer branch diagnostics retained after the first discovery pass.
#[derive(Clone, Debug)]
pub(super) enum FileLayerTrace {
    /// An explicit CLI or environment selector chose a configuration path.
    Explicit {
        /// The explicitly selected configuration path.
        path: BoundedConfigPath,
    },
    /// Selector-free discovery, with any project-scope second-pass outcome.
    Automatic {
        /// Trail of the project-scope second pass, when one ran.
        project_scope: Option<ProjectScopeTrace>,
    },
}

impl FileLayerTrace {
    /// Emit the selected layer-collection branch without filesystem access.
    fn emit(&self) {
        match self {
            Self::Explicit { path } => {
                debug_config_path_from_fields("using explicit config path", path);
            }
            Self::Automatic { project_scope } => {
                debug!("using config discovery");
                if let Some(trace) = project_scope {
                    trace.emit();
                }
            }
        }
    }
}

/// Diagnostics deferred until a composition boundary enables its filter.
#[derive(Clone, Debug)]
pub(super) struct DiscoveryDiagnostics {
    /// Discovery events to emit on replay.
    trace: DiscoveryTrace,
    /// Warning to emit on replay when an explicit config load failed.
    load_warning: Option<ConfigLoadWarning>,
    /// Bounded environment-grant quarantine summary retained from layer resolution.
    env_grant_quarantine: EnvGrantQuarantineTrace,
}

impl DiscoveryDiagnostics {
    /// Combine bounded discovery events and any explicit-load warning.
    pub(super) fn new(trace: DiscoveryTrace, load_warning: Option<ConfigLoadWarning>) -> Self {
        Self {
            trace,
            load_warning,
            env_grant_quarantine: EnvGrantQuarantineTrace::default(),
        }
    }

    /// Retain bounded environment-grant quarantine telemetry for normal replay.
    pub(super) const fn with_env_grant_quarantine(
        mut self,
        env_grant_quarantine: EnvGrantQuarantineTrace,
    ) -> Self {
        self.env_grant_quarantine = env_grant_quarantine;
        self
    }

    /// Emit deferred diagnostics without querying the environment or filesystem.
    pub(super) fn emit(&self) {
        self.trace.emit();
        self.env_grant_quarantine.emit();
        if let Some(warning) = &self.load_warning {
            warning.emit();
        }
    }
}
