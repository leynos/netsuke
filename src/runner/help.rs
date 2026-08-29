//! Dispatch and rendering for the `netsuke help` subcommand.
//!
//! The `help targets` topic loads, expands, renders, and validates the selected
//! manifest without invoking Ninja, then prints a deterministic catalogue of
//! actions and targets with their descriptions. The no-topic and
//! subcommand-name topics render clap's localized help text instead.

use anyhow::{Context, Result};
use serde::Serialize;
use std::borrow::Cow;
use unicode_width::UnicodeWidthStr;

use crate::cli::{Cli, configured_command};
use crate::json_envelope::{GeneratorInfo, SCHEMA_VERSION};
use crate::localization::{self, keys};
use crate::output_mode;
use crate::output_prefs::{self, OutputPrefs};
use crate::status::{LocalizationKey, PipelineStage, StatusReporter, report_pipeline_stage};
use crate::theme::ThemeContext;

use super::process;
use telemetry::instrument_help_targets;

#[path = "help_query.rs"]
mod query;
#[path = "help_telemetry.rs"]
mod telemetry;

#[cfg(test)]
use query::build_catalogue;
use query::{HelpEntry, HelpTargetsQueryFailure, query_help_targets};

/// Render the `help targets` catalogue to stdout without invoking Ninja.
///
/// The manifest is loaded, expanded, rendered, and validated through the same
/// pipeline stages as a real build, but with impure template helpers disabled.
/// The IR is built only to validate the rendered manifest, and no recipe is
/// executed and no build output created.
///
/// # Errors
///
/// Returns an error when the manifest cannot be resolved, loaded, rendered, or
/// validated, or when the catalogue cannot be serialized.
pub(super) fn handle_help_targets(cli: &Cli, reporter: &dyn StatusReporter) -> Result<()> {
    let query =
        match instrument_help_targets(|| query_help_targets(cli).map_err(anyhow::Error::new)) {
            Ok(query) => query,
            Err(error) => {
                report_query_failure_stages(reporter, &error);
                return Err(error);
            }
        };
    report_query_stages(reporter, &query.stages);
    report_pipeline_stage(reporter, PipelineStage::IrGenerationValidation, None);
    let status_key: LocalizationKey = keys::STATUS_TOOL_HELP_TARGETS.into();
    report_pipeline_stage(reporter, PipelineStage::GraphRendering, Some(status_key));
    if cli.json {
        let rendered = render_json(&query.entries)?;
        process::write_text_stdout(&rendered)?;
    } else {
        let rendered = render_text(&query.entries, resolved_prefs(cli));
        process::write_text_stdout(&rendered)?;
    }
    reporter.report_complete(status_key);
    Ok(())
}

/// Emit the loading stages returned by the pure catalogue query.
fn report_query_stages(reporter: &dyn StatusReporter, stages: &[PipelineStage]) {
    for stage in stages {
        report_pipeline_stage(reporter, *stage, None);
    }
}

/// Emit stages accumulated before a pure catalogue query failed.
fn report_query_failure_stages(reporter: &dyn StatusReporter, error: &anyhow::Error) {
    if let Some(failure) = error.downcast_ref::<HelpTargetsQueryFailure>() {
        report_query_stages(reporter, &failure.stages);
    }
}

/// Render the localized top-level long help, matching `--help`.
///
/// # Errors
///
/// Returns an error when the help text cannot be written to stdout.
pub(super) fn render_root_help() -> Result<()> {
    let localizer = localization::localizer();
    let mut command = configured_command(Some(&localizer));
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
    let mut command = configured_command(Some(&localizer));
    let subcommand = command
        .find_subcommand_mut(name)
        .with_context(|| format!("unknown subcommand '{name}'"))?;
    let text = subcommand.render_long_help().to_string();
    process::write_text_stdout(&text)
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
/// marker and a localized conditional marker. A missing description leaves the
/// entry visible without a description column. Empty sections are omitted.
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

/// Render one aligned catalogue section, omitting it entirely when empty.
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
    let display_names: Vec<Cow<'_, str>> = entries
        .iter()
        .map(|entry| terminal_safe(&entry.name))
        .collect();
    let width = display_names
        .iter()
        .map(|name| UnicodeWidthStr::width(name.as_ref()))
        .max()
        .unwrap_or(0);
    let default_marker = default_marker(prefs);
    let conditional_marker = conditional_marker(prefs);
    for (entry, name) in entries.iter().zip(display_names) {
        let name_width = UnicodeWidthStr::width(name.as_ref());
        out.push_str("  ");
        out.push_str(&name);
        out.push_str(&" ".repeat(width.saturating_sub(name_width)));
        if let Some(description) = entry.description.as_deref() {
            out.push_str("  ");
            out.push_str(&terminal_safe(description));
        }
        if entry.is_default {
            out.push(' ');
            out.push_str(&default_marker);
        }
        if entry.conditional {
            out.push(' ');
            out.push_str(&conditional_marker);
        }
        out.push('\n');
    }
}

/// Render manifest-controlled text safely for a terminal.
///
/// Catalogue names and descriptions can contain arbitrary rendered template
/// values.  Keep printable Unicode intact, while making every control
/// character visible so a manifest cannot inject terminal controls or rows.
fn terminal_safe(input: &str) -> Cow<'_, str> {
    if !input.chars().any(is_terminal_control) {
        return Cow::Borrowed(input);
    }

    let mut escaped = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            control if is_terminal_control(control) => escaped.extend(control.escape_unicode()),
            printable => escaped.push(printable),
        }
    }
    Cow::Owned(escaped)
}

/// Return whether a character can control terminal display or reading order.
const fn is_terminal_control(character: char) -> bool {
    matches!(
        character,
        '\0'..='\u{001F}'
            | '\u{007F}'..='\u{009F}'
            | '\u{061C}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
    )
}

/// The localized default marker, pairing a theme glyph with a translated label
/// so the meaning never depends on the glyph alone.
fn default_marker(prefs: OutputPrefs) -> String {
    let glyph = if prefs.emoji_allowed() { "★" } else { "*" };
    let label = localization::message(keys::CLI_HELP_DEFAULT_MARKER).to_string();
    format!("[{glyph} {label}]")
}

/// The localized conditional marker, pairing a theme glyph with a translated
/// label so entries retained without evaluating their `when` condition are
/// never mistaken for unconditionally available targets.
fn conditional_marker(prefs: OutputPrefs) -> String {
    let glyph = if prefs.emoji_allowed() { "◇" } else { "?" };
    let label = localization::message(keys::CLI_HELP_CONDITIONAL_MARKER).to_string();
    format!("[{glyph} {label}]")
}
/// Versioned JSON catalogue document, mirroring `crate::result_json`'s
/// envelope shape while carrying the listing payload instead of free text.
#[derive(Debug, Serialize)]
struct HelpTargetsDocument<'a> {
    /// Schema version of the JSON result envelope.
    schema_version: u32,
    /// Versioned generator details for the envelope.
    generator: GeneratorInfo,
    /// The catalogue payload of the result document.
    result: HelpTargetsResult<'a>,
}

#[derive(Debug, Serialize)]
/// Rendered actions and targets sections of the help-targets catalogue.
struct HelpTargetsResult<'a> {
    /// Name of the producing subcommand, identifying the payload.
    command: &'static str,
    /// JSON rows for the manifest action targets.
    actions: Vec<HelpEntryJson<'a>>,
    /// JSON rows for the manifest build targets.
    targets: Vec<HelpEntryJson<'a>>,
}

#[derive(Debug, Serialize)]
/// One serialized catalogue row for the JSON help-targets document.
struct HelpEntryJson<'a> {
    /// Resolved target name.
    name: &'a str,
    /// Manifest-controlled description, when present.
    description: Option<&'a str>,
    /// Whether the entry is one of the manifest's default targets.
    default: bool,
    /// Whether the entry has an unevaluated query-disabled condition.
    conditional: bool,
}

/// Serialize the catalogue into a pretty-printed versioned JSON document.
///
/// # Errors
///
/// Returns an error when the catalogue cannot be serialized.
fn render_json(entries: &[HelpEntry]) -> Result<String> {
    serde_json::to_string_pretty(&HelpTargetsDocument {
        schema_version: SCHEMA_VERSION,
        generator: GeneratorInfo::current(),
        result: HelpTargetsResult {
            command: "help-targets",
            actions: json_entries(entries.iter().filter(|entry| entry.is_action)),
            targets: json_entries(entries.iter().filter(|entry| !entry.is_action)),
        },
    })
    .context("serialize help targets catalogue")
}

/// Project catalogue entries into the JSON row shape, retaining only safe fields.
fn json_entries<'entry>(
    entries: impl Iterator<Item = &'entry HelpEntry>,
) -> Vec<HelpEntryJson<'entry>> {
    entries
        .into_iter()
        .map(|entry| HelpEntryJson {
            name: entry.name.as_str(),
            description: entry.description.as_deref(),
            default: entry.is_default,
            conditional: entry.conditional,
        })
        .collect()
}

#[cfg(test)]
#[path = "help_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "help_telemetry_tests.rs"]
mod telemetry_tests;
