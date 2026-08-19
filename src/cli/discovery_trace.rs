//! Bounded metadata for replaying configuration discovery diagnostics.
//!
//! Discovery captures this data while it still owns the selected paths and
//! project-scope result. Startup later replays the original trace events after
//! it enables verbose output, without accessing the environment or filesystem.

use tracing::debug;

use super::ConfigPathResolution;
use super::diagnostics::{
    BoundedConfigPath, ConfigLoadWarning, debug_config_path_from_fields,
    trace_config_path_variable_from_fields,
};
use super::layers::ProjectScopeTrace;

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
    Explicit { path: BoundedConfigPath },
    /// Selector-free discovery, with any project-scope second-pass outcome.
    Automatic {
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
}

impl DiscoveryDiagnostics {
    /// Combine bounded discovery events and any explicit-load warning.
    pub(super) const fn new(
        trace: DiscoveryTrace,
        load_warning: Option<ConfigLoadWarning>,
    ) -> Self {
        Self {
            trace,
            load_warning,
        }
    }

    /// Emit deferred diagnostics without querying the environment or filesystem.
    pub(super) fn emit(&self) {
        self.trace.emit();
        if let Some(warning) = &self.load_warning {
            warning.emit();
        }
    }
}
