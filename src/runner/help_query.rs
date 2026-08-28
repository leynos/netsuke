//! Pure manifest loading and catalogue construction for `netsuke help targets`.

use anyhow::{Context, Result, bail, ensure};
use std::{collections::HashSet, error::Error as StdError, fmt, sync::Arc};

use crate::ast::{NetsukeManifest, Target};
use crate::cli::Cli;
use crate::ir::BuildGraph;
use crate::localization::{self, keys};
use crate::status::PipelineStage;

use super::super::RunnerError;
use super::super::generation;
use super::super::path_helpers::{ensure_manifest_exists, resolve_manifest_path};
use super::terminal_safe;

/// One catalogue row: a single resolved target name with its metadata.
pub(super) struct HelpEntry {
    /// Resolved target name, one row per declared name.
    pub(super) name: String,
    /// Manifest-controlled description, when present.
    pub(super) description: Option<Arc<str>>,
    /// Whether the entry comes from the manifest's `actions` section.
    pub(super) is_action: bool,
    /// Whether the entry is one of the manifest's default targets.
    pub(super) is_default: bool,
    /// Whether query-disabled `when` evaluation left the entry conditional.
    pub(super) conditional: bool,
}

/// The pure result of loading, validating, and cataloguing a help manifest.
pub(super) struct HelpTargetsQuery {
    /// Catalogue rows in declaration order, actions first.
    pub(super) entries: Vec<HelpEntry>,
    /// Pipeline stages traversed while loading the manifest.
    pub(super) stages: Vec<PipelineStage>,
}

/// A query failure with stages for the command boundary to report.
#[derive(Debug)]
pub(super) struct HelpTargetsQueryFailure {
    /// The underlying failure, displayable as its own message.
    pub(super) error: anyhow::Error,
    /// Stages traversed before the failure, for the command boundary to report.
    pub(super) stages: Vec<PipelineStage>,
}

impl fmt::Display for HelpTargetsQueryFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl StdError for HelpTargetsQueryFailure {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.error.as_ref())
    }
}

/// Load, validate, and catalogue manifest discovery metadata without effects.
pub(super) fn query_help_targets(
    cli: &Cli,
) -> std::result::Result<HelpTargetsQuery, HelpTargetsQueryFailure> {
    let mut stages = Vec::new();
    let result = query_entries(cli, &mut stages);
    match result {
        Ok(entries) => Ok(HelpTargetsQuery { entries, stages }),
        Err(error) => Err(HelpTargetsQueryFailure { error, stages }),
    }
}

/// Load, validate, and flatten the manifest into the catalogue row list.
///
/// The manifest is rendered and validated through a no-side-effect load that
/// builds the IR only to catch duplicate outputs, missing rules, and cycles.
///
/// # Errors
///
/// Returns an error when the manifest cannot be resolved, loaded, rendered, or
/// validated, or when a declared default names no catalogue entry.
fn query_entries(cli: &Cli, stages: &mut Vec<PipelineStage>) -> Result<Vec<HelpEntry>> {
    let manifest_path = resolve_manifest_path(cli)?;
    if let Err(error) = ensure_manifest_exists(cli, &manifest_path) {
        record_missing_manifest_stage(&error, stages);
        return Err(error);
    }
    let manifest = load_manifest_for_query(&manifest_path, stages)?;
    reject_terminal_controls_in_target_names(&manifest)?;
    let entries = build_catalogue(&manifest);
    let defaults = manifest.defaults.clone();

    // Building the IR validates every resolved entry (duplicate outputs,
    // missing rules, cycles) without generating Ninja or executing recipes.
    // Conditional entries are mutually exclusive alternatives whose `when`
    // expressions manifest-query mode cannot inspect, so validate only the
    // resolved subset rather than treating them as simultaneously active.
    BuildGraph::from_manifest(&manifest_for_graph_validation(manifest))
        .context(localization::message(keys::RUNNER_CONTEXT_BUILD_GRAPH))?;

    validate_defaults(&defaults, &entries)?;
    Ok(entries)
}

/// Clone a manifest for graph validation without unresolved conditional entries.
///
/// Query-disabled `when` helpers leave alternatives in the catalogue. The
/// build graph must not validate those alternatives as coexisting outputs.
fn manifest_for_graph_validation(mut manifest: NetsukeManifest) -> NetsukeManifest {
    manifest.actions.retain(|target| !target.conditional);
    manifest.targets.retain(|target| !target.conditional);
    manifest
}

/// Reject control-bearing target names before IR errors can interpolate them.
///
/// The help query returns validation diagnostics directly to the terminal.
/// Catalogue rendering escapes controls, but graph validation happens first and
/// can include a target name in its localized errors. Reject such names before
/// that boundary rather than allowing them to influence a diagnostic.
fn reject_terminal_controls_in_target_names(manifest: &NetsukeManifest) -> Result<()> {
    let targets = manifest.actions.iter().chain(&manifest.targets);
    if targets
        .flat_map(|target| target.name.to_string_vec())
        .any(|name| name.chars().any(super::is_terminal_control))
    {
        bail!("help targets cannot validate a target name with terminal control characters");
    }
    Ok(())
}

/// Record the ingestion stage when the failure reports a missing manifest.
fn record_missing_manifest_stage(error: &anyhow::Error, stages: &mut Vec<PipelineStage>) {
    if error
        .downcast_ref::<RunnerError>()
        .is_some_and(|runner_error| matches!(runner_error, RunnerError::ManifestNotFound { .. }))
    {
        stages.push(PipelineStage::ManifestIngestion);
    }
}

/// Flatten the rendered manifest into a deterministic catalogue in declaration
/// order: actions first, then targets. A multi-name entry yields one row per
/// name, each carrying the same description and default status.
pub(super) fn build_catalogue(manifest: &NetsukeManifest) -> Vec<HelpEntry> {
    let mut entries = Vec::new();
    let defaults: HashSet<&str> = manifest.defaults.iter().map(String::as_str).collect();
    for target in &manifest.actions {
        append_target_entries(&mut entries, target, true, &defaults);
    }
    for target in &manifest.targets {
        append_target_entries(&mut entries, target, false, &defaults);
    }
    entries
}

/// Ensure every declared default names a catalogue entry.
///
/// # Errors
///
/// Returns an error when a declared default does not appear among the entries.
fn validate_defaults(defaults: &[String], entries: &[HelpEntry]) -> Result<()> {
    let names: HashSet<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
    for default in defaults {
        let safe_default = terminal_safe(default);
        ensure!(
            names.contains(default.as_str()),
            localization::message(keys::RUNNER_MANIFEST_DEFAULT_NOT_DECLARED)
                .with_arg("default", safe_default.as_ref())
        );
    }
    Ok(())
}

/// Append one catalogue row per declared target name, sharing its metadata.
fn append_target_entries(
    entries: &mut Vec<HelpEntry>,
    target: &Target,
    is_action: bool,
    defaults: &HashSet<&str>,
) {
    let description = target.description.as_deref().map(Arc::<str>::from);
    for name in target.name.to_string_vec() {
        entries.push(HelpEntry {
            is_default: defaults.contains(name.as_str()),
            conditional: target.conditional,
            name,
            description: description.clone(),
            is_action,
        });
    }
}

/// Load a manifest for a no-side-effect metadata query and retain its stages.
fn load_manifest_for_query(
    manifest_path: &camino::Utf8PathBuf,
    stages: &mut Vec<PipelineStage>,
) -> Result<NetsukeManifest> {
    let mut on_stage = |stage| stages.push(pipeline_stage(stage));
    generation::load_manifest(manifest_path, Some(&mut on_stage))
}

/// Map manifest-loading events to data that the command boundary can report.
const fn pipeline_stage(stage: crate::manifest::ManifestLoadStage) -> PipelineStage {
    match stage {
        crate::manifest::ManifestLoadStage::ManifestIngestion => PipelineStage::ManifestIngestion,
        crate::manifest::ManifestLoadStage::InitialYamlParsing => PipelineStage::InitialYamlParsing,
        crate::manifest::ManifestLoadStage::TemplateExpansion => PipelineStage::TemplateExpansion,
        crate::manifest::ManifestLoadStage::FinalRendering => PipelineStage::FinalRendering,
    }
}
