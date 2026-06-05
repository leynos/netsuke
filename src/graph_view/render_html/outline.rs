//! Accessible plain-text outline of targets and their inputs.
//!
//! The outline is the screen-reader path while structured `--json` inspection
//! (roadmap 3.15.6) remains a follow-up. It is emitted inside a
//! `<details open>` block so a sighted user sees the same data without
//! interaction.

use std::collections::BTreeMap;
use std::io::Write;

use camino::Utf8Path;

use crate::graph_view::render::GraphRenderError;
use crate::graph_view::{GraphView, NodeKind};
use crate::localization::keys;

use super::escape::escape_text;
use super::layout::collect_predecessors;
use super::localized;

pub(super) fn write_outline(
    sink: &mut dyn Write,
    view: &GraphView,
    summary: &str,
) -> Result<(), GraphRenderError> {
    let no_inputs = localized(keys::GRAPH_HTML_OUTLINE_NO_INPUTS);
    let inputs_by_target = collect_inputs_by_target(view);
    writeln!(sink, "  <details open>")?;
    writeln!(sink, "    <summary>{}</summary>", escape_text(summary))?;
    writeln!(sink, "    <ul>")?;
    for node in &view.nodes {
        if let NodeKind::Target { .. } = node.kind {
            let inputs = inputs_by_target
                .get(node.path.as_path())
                .map_or(&[] as &[&Utf8Path], Vec::as_slice);
            write_outline_item(sink, node.path.as_str(), inputs, &no_inputs)?;
        }
    }
    writeln!(sink, "    </ul>")?;
    writeln!(sink, "  </details>")?;
    Ok(())
}

/// Write the `<li>` block for a single target node in the outline.
fn write_outline_item(
    sink: &mut dyn Write,
    target: &str,
    inputs: &[&Utf8Path],
    no_inputs_msg: &str,
) -> Result<(), GraphRenderError> {
    writeln!(sink, "      <li>")?;
    writeln!(sink, "        <code>{}</code>", escape_text(target))?;
    write_outline_inputs(sink, inputs, no_inputs_msg)?;
    writeln!(sink, "      </li>")?;
    Ok(())
}

/// Write the inputs sub-list, or a "no inputs" notice if the slice is empty.
fn write_outline_inputs(
    sink: &mut dyn Write,
    inputs: &[&Utf8Path],
    no_inputs_msg: &str,
) -> Result<(), GraphRenderError> {
    if inputs.is_empty() {
        writeln!(sink, "        <p>{}</p>", escape_text(no_inputs_msg))?;
        return Ok(());
    }
    writeln!(sink, "        <ul>")?;
    for input in inputs {
        writeln!(
            sink,
            "          <li><code>{}</code></li>",
            escape_text(input.as_str())
        )?;
    }
    writeln!(sink, "        </ul>")?;
    Ok(())
}

fn collect_inputs_by_target(view: &GraphView) -> BTreeMap<&Utf8Path, Vec<&Utf8Path>> {
    collect_predecessors(&view.edges)
}
