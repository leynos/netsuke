//! String adapters around the streamed catalogue renderers.
//!
//! Production writes each row straight to stdout. Snapshot and structural
//! assertions need the same bytes in memory, so these adapters render into a
//! buffer and decode it as UTF-8.

use super::super::{HelpEntry, OutputPrefs, graph_json, render_json, render_text};
use anyhow::{Context, Result};

/// Render the streamed text catalogue into a string for assertions.
///
/// # Errors
///
/// Returns an error when a row cannot be rendered or the bytes are not UTF-8.
pub(super) fn render_text_string(entries: &[HelpEntry], prefs: OutputPrefs) -> Result<String> {
    let mut buffer = Vec::new();
    render_text(&mut buffer, entries, prefs)?;
    String::from_utf8(buffer).context("text catalogue should be valid UTF-8")
}

/// Render the streamed JSON catalogue into a string for assertions.
///
/// # Errors
///
/// Returns an error when the document cannot be serialized or the bytes are
/// not UTF-8.
pub(super) fn render_json_string(
    entries: &[HelpEntry],
    graph: Option<&graph_json::GraphJson>,
) -> Result<String> {
    let mut buffer = Vec::new();
    render_json(&mut buffer, entries, graph)?;
    String::from_utf8(buffer).context("JSON catalogue should be valid UTF-8")
}
