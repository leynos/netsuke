//! Dispatch and rendering for the `netsuke help` subcommand.
//!
//! The `help targets` topic loads, expands, renders, and validates the selected
//! manifest without invoking Ninja, then prints a deterministic catalogue of
//! actions and targets with their descriptions. The no-topic and
//! subcommand-name topics render clap's localized help text instead.

use anyhow::{Context, Result};
use clap::CommandFactory;
use serde::Serialize;
use tracing::info;
use unicode_width::UnicodeWidthStr;

use crate::ast::{NetsukeManifest, Target};
use crate::cli::Cli;
use crate::cli_l10n::localize_command;
use crate::ir::BuildGraph;
use crate::json_envelope::{GeneratorInfo, SCHEMA_VERSION};
use crate::localization::{self, keys};
use crate::output_mode;
use crate::output_prefs::{self, OutputPrefs};
use crate::status::{LocalizationKey, PipelineStage, StatusReporter, report_pipeline_stage};
use crate::theme::ThemeContext;

use super::path_helpers::{ensure_manifest_exists_or_error, resolve_manifest_path};
use super::{load_manifest_with_stage_reporting, process};

/// One catalogue row: a single resolved target name with its metadata.
struct HelpEntry {
    name: String,
    description: Option<String>,
    is_action: bool,
    is_default: bool,
}

/// Render the `help targets` catalogue to stdout without invoking Ninja.
///
/// The manifest is loaded, expanded, rendered, and validated through the same
/// pipeline stages as a real build; the IR is built only to validate the
/// rendered manifest, and no recipe is executed and no build output created.
///
/// # Errors
///
/// Returns an error when the manifest cannot be resolved, loaded, rendered, or
/// validated, or when the catalogue cannot be serialized.
pub(super) fn handle_help_targets(cli: &Cli, reporter: &dyn StatusReporter) -> Result<()> {
    info!(
        target: "netsuke::subcommand",
        subcommand = "help-targets",
        "Rendering target and action catalogue"
    );
    let manifest_path = resolve_manifest_path(cli)?;
    ensure_manifest_exists_or_error(cli, reporter, &manifest_path)?;
    let policy = cli
        .network_policy()
        .context(localization::message(keys::RUNNER_CONTEXT_NETWORK_POLICY))?;
    let manifest = load_manifest_with_stage_reporting(&manifest_path, policy, reporter)?;

    report_pipeline_stage(reporter, PipelineStage::IrGenerationValidation, None);
    // Building the IR validates the rendered manifest (duplicate outputs,
    // missing rules, cycles) exactly as a real build would, without generating
    // Ninja or executing any recipe.
    BuildGraph::from_manifest(&manifest)
        .context(localization::message(keys::RUNNER_CONTEXT_BUILD_GRAPH))?;

    let status_key: LocalizationKey = keys::STATUS_TOOL_HELP_TARGETS.into();
    report_pipeline_stage(reporter, PipelineStage::GraphRendering, Some(status_key));

    let entries = build_catalogue(&manifest);
    if cli.json {
        let rendered = render_json(&entries).context("serialize help targets catalogue")?;
        process::write_text_stdout(&rendered)?;
    } else {
        let rendered = render_text(&entries, resolved_prefs(cli));
        process::write_text_stdout(&rendered)?;
    }
    reporter.report_complete(status_key);
    Ok(())
}

/// Render the localized top-level long help, matching `--help`.
///
/// # Errors
///
/// Returns an error when the help text cannot be written to stdout.
pub(super) fn render_root_help() -> Result<()> {
    let localizer = localization::localizer();
    let mut command = localize_command(Cli::command(), localizer.as_ref());
    let text = command.render_long_help().to_string();
    process::write_text_stdout(&text)
}

/// Render the localized long help for a named subcommand.
///
/// # Errors
///
/// Returns an error when the subcommand is unknown or the help text cannot be
/// written to stdout.
pub(super) fn render_subcommand_help(name: &str) -> Result<()> {
    let localizer = localization::localizer();
    let mut command = localize_command(Cli::command(), localizer.as_ref());
    let subcommand = command
        .find_subcommand_mut(name)
        .with_context(|| format!("unknown subcommand '{name}'"))?;
    let text = subcommand.render_long_help().to_string();
    process::write_text_stdout(&text)
}

/// Flatten the rendered manifest into a deterministic catalogue in declaration
/// order: actions first, then targets. A multi-name entry yields one row per
/// name, each carrying the same description and default status.
fn build_catalogue(manifest: &NetsukeManifest) -> Vec<HelpEntry> {
    let mut entries = Vec::new();
    for target in &manifest.actions {
        append_target_entries(&mut entries, target, true, &manifest.defaults);
    }
    for target in &manifest.targets {
        append_target_entries(&mut entries, target, false, &manifest.defaults);
    }
    entries
}

fn append_target_entries(
    entries: &mut Vec<HelpEntry>,
    target: &Target,
    is_action: bool,
    defaults: &[String],
) {
    for name in target.name.to_string_vec() {
        entries.push(HelpEntry {
            is_default: defaults.iter().any(|default| default == &name),
            name,
            description: target.description.clone(),
            is_action,
        });
    }
}

/// Resolve the same output preferences the rest of the CLI uses, so emoji and
/// accessibility settings drive the catalogue's marker glyph.
fn resolved_prefs(cli: &Cli) -> OutputPrefs {
    let mode = output_mode::resolve(cli.accessibility_override(), Some(cli.color));
    output_prefs::resolve_from_theme(
        cli.theme_preference(),
        ThemeContext::new(None, Some(cli.color), mode),
    )
}

/// Render the text catalogue: an "Actions:" section followed by a "Targets:"
/// section, with aligned name and description columns and a localized default
/// marker. A missing description stays an empty column so the entry is never
/// hidden. Empty sections are omitted.
fn render_text(entries: &[HelpEntry], prefs: OutputPrefs) -> String {
    let actions: Vec<&HelpEntry> = entries.iter().filter(|entry| entry.is_action).collect();
    let targets: Vec<&HelpEntry> = entries.iter().filter(|entry| !entry.is_action).collect();
    let mut out = String::new();
    render_section(&mut out, &actions, keys::CLI_HELP_ACTIONS_HEADING, prefs);
    if !actions.is_empty() && !targets.is_empty() {
        out.push('\n');
    }
    render_section(&mut out, &targets, keys::CLI_HELP_TARGETS_HEADING, prefs);
    out
}

fn render_section(
    out: &mut String,
    entries: &[&HelpEntry],
    heading_key: &'static str,
    prefs: OutputPrefs,
) {
    if entries.is_empty() {
        return;
    }
    out.push_str(&localization::message(heading_key).to_string());
    out.push('\n');
    let width = entries
        .iter()
        .map(|entry| UnicodeWidthStr::width(entry.name.as_str()))
        .max()
        .unwrap_or(0);
    let marker = default_marker(prefs);
    for entry in entries {
        let name_width = UnicodeWidthStr::width(entry.name.as_str());
        out.push_str("  ");
        out.push_str(&entry.name);
        out.push_str(&" ".repeat(width.saturating_sub(name_width)));
        out.push_str("  ");
        if let Some(description) = &entry.description {
            out.push_str(description);
        }
        if entry.is_default {
            out.push(' ');
            out.push_str(&marker);
        }
        out.push('\n');
    }
}

/// The localized default marker, pairing a theme glyph with a translated label
/// so the meaning never depends on the glyph alone.
fn default_marker(prefs: OutputPrefs) -> String {
    let glyph = if prefs.emoji_allowed() { "★" } else { "*" };
    let label = localization::message(keys::CLI_HELP_DEFAULT_MARKER).to_string();
    format!("[{glyph} {label}]")
}

/// Versioned JSON catalogue document, mirroring `crate::result_json`'s
/// envelope shape while carrying the listing payload instead of free text.
#[derive(Debug, Serialize)]
struct HelpTargetsDocument<'a> {
    schema_version: u32,
    generator: GeneratorInfo,
    result: HelpTargetsResult<'a>,
}

#[derive(Debug, Serialize)]
struct HelpTargetsResult<'a> {
    command: &'static str,
    actions: Vec<HelpEntryJson<'a>>,
    targets: Vec<HelpEntryJson<'a>>,
}

#[derive(Debug, Serialize)]
struct HelpEntryJson<'a> {
    name: &'a str,
    description: Option<&'a str>,
    default: bool,
}

fn render_json(entries: &[HelpEntry]) -> Result<String> {
    serde_json::to_string_pretty(&HelpTargetsDocument {
        schema_version: SCHEMA_VERSION,
        generator: GeneratorInfo::current(),
        result: HelpTargetsResult {
            command: "help-targets",
            actions: json_entries(entries, true),
            targets: json_entries(entries, false),
        },
    })
    .context("serialize help targets catalogue")
}

fn json_entries(entries: &[HelpEntry], is_action: bool) -> Vec<HelpEntryJson<'_>> {
    entries
        .iter()
        .filter(|entry| entry.is_action == is_action)
        .map(|entry| HelpEntryJson {
            name: &entry.name,
            description: entry.description.as_deref(),
            default: entry.is_default,
        })
        .collect()
}

#[cfg(test)]
#[path = "help_tests.rs"]
mod tests;
