//! Inline CSS stylesheet for the HTML graph view.
//!
//! No external resources are referenced; the renderer keeps the document
//! offline-safe by inlining everything required to display the SVG, outline,
//! and `<noscript>` fallback.

use std::io::Write;

use crate::graph_view::render::GraphRenderError;

pub(super) fn write_inline_style(sink: &mut dyn Write) -> Result<(), GraphRenderError> {
    writeln!(sink, "  <style>")?;
    writeln!(
        sink,
        "    :root {{ color-scheme: light dark; font-family: system-ui, sans-serif; }}"
    )?;
    writeln!(
        sink,
        "    body {{ margin: 1.5rem; max-width: 100%; line-height: 1.5; }}"
    )?;
    writeln!(sink, "    svg {{ max-width: 100%; height: auto; }}")?;
    writeln!(
        sink,
        "    .node rect {{ fill: var(--node-bg, #f3f4f6); stroke: #003366; stroke-width: 1.2; rx: 6; ry: 6; }}"
    )?;
    writeln!(
        sink,
        "    .node.source rect {{ stroke: #444; stroke-dasharray: 3 2; }}"
    )?;
    writeln!(
        sink,
        "    .node.phony rect {{ stroke: #666; stroke-dasharray: 4 2; }}"
    )?;
    writeln!(sink, "    .node.always rect {{ stroke: #cc6600; }}")?;
    writeln!(
        sink,
        "    .node text {{ font-size: 12px; fill: currentColor; pointer-events: none; }}"
    )?;
    writeln!(
        sink,
        "    .edge {{ fill: none; stroke: #003366; stroke-width: 1.2; }}"
    )?;
    writeln!(
        sink,
        "    .edge.order-only {{ stroke: #888; stroke-dasharray: 4 3; }}"
    )?;
    writeln!(
        sink,
        "    .edge.implicit-output {{ stroke: #003366; stroke-dasharray: 2 2; }}"
    )?;
    writeln!(
        sink,
        "    .edge.implicit-dep {{ stroke: #003366; stroke-width: 2.4; }}"
    )?;
    writeln!(
        sink,
        "    details {{ margin-top: 1rem; }} details > summary {{ cursor: pointer; font-weight: 600; }}"
    )?;
    writeln!(
        sink,
        "    pre {{ overflow: auto; padding: 0.5rem; background: #f3f4f6; }}"
    )?;
    writeln!(sink, "  </style>")?;
    Ok(())
}
