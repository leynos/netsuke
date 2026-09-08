//! Input and accumulation state for one cached configuration merge.
//!
//! The application creates a [`CachedMergeInput`] after diagnostic discovery,
//! while the merge implementation owns [`MergeComposition`] until schema
//! validation produces the resolved configuration.

use clap::ArgMatches;
use ortho_config::declarative::LayerComposition;
use ortho_config::{MergeComposer, OrthoError, OrthoResult};
use std::sync::Arc;

use super::command::Cli;
use super::config::CliConfig;
use super::discovery::{DiscoveredLayers, EnvProvider, ProjectManifestBudgetRequest};

/// Inputs for one cached configuration merge, owned by its application caller.
pub struct CachedMergeInput<'a, E: ?Sized> {
    /// Parsed command-line configuration before layered resolution.
    pub(super) cli: &'a Cli,
    /// Clap matches used to identify explicitly supplied overrides.
    pub(super) matches: &'a ArgMatches,
    /// Injected environment for discovery and configuration-layer extraction.
    pub(super) env: &'a E,
    /// Cached file layers transferred from one discovery pass.
    pub(super) discovered: DiscoveredLayers,
}

impl<'a, E> CachedMergeInput<'a, E>
where
    E: EnvProvider + ?Sized,
{
    /// Create one merge request from parsed input and previously discovered layers.
    pub const fn new(
        cli: &'a Cli,
        matches: &'a ArgMatches,
        env: &'a E,
        discovered: DiscoveredLayers,
    ) -> Self {
        Self {
            cli,
            matches,
            env,
            discovered,
        }
    }
}

/// Accumulates merge layers and errors before applying the configuration schema.
pub(super) struct MergeComposition {
    /// Ordered layer collector for the four-layer configuration merge.
    pub(super) composer: MergeComposer,
    /// Deferred layer-construction failures collected before schema validation.
    pub(super) errors: Vec<Arc<OrthoError>>,
    /// Project budget restrictions awaiting trust-aware reconciliation.
    pub(super) project_manifest_budget_request: ProjectManifestBudgetRequest,
}

impl MergeComposition {
    /// Start a four-layer configuration composition.
    pub(super) fn new() -> Self {
        Self {
            composer: MergeComposer::with_capacity(4),
            errors: Vec::new(),
            project_manifest_budget_request: ProjectManifestBudgetRequest::default(),
        }
    }

    /// Apply the schema after all layers and layer errors have been collected.
    pub(super) fn into_merge_result(self) -> OrthoResult<CliConfig> {
        LayerComposition::new(self.composer.layers(), self.errors)
            .into_merge_result(CliConfig::merge_from_layers)
    }
}
