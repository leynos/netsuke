//! Self-contained HTML renderer for [`GraphView`].
//!
//! Produces a single offline-safe HTML document containing:
//!
//! 1. A server-rendered SVG laid out as a left-to-right hierarchical DAG.
//!    The SVG carries `role="img"`, `aria-labelledby`, per-node `aria-label`s,
//!    and per-edge `<title>` elements so screen readers can navigate the
//!    structure.
//! 2. A `<details>` block listing every target and its inputs as a plain-text
//!    outline — the primary screen-reader path while structured `--json`
//!    inspection (roadmap 3.15.6) remains a follow-up.
//! 3. A `<noscript>` block restating the dependency graph and the DOT source
//!    verbatim so the page is fully functional with JavaScript disabled.
//!
//! No JavaScript is required at view time. No external assets are referenced.

use std::collections::BTreeMap;
use std::io::Write;

use camino::Utf8Path;

use crate::graph_view::render_dot::DotRenderer;
use crate::localization::{self, keys};

use super::render::{GraphRenderError, GraphRenderer};
use super::{EdgeClass, EdgeView, GraphView, NodeKind, NodeView};

const COL_WIDTH: i32 = 240;
const ROW_HEIGHT: i32 = 70;
const NODE_WIDTH: i32 = 200;
const NODE_HEIGHT: i32 = 44;
const NODE_HEIGHT_HALF: i32 = NODE_HEIGHT >> 1;
const MARGIN: i32 = 24;

/// Render adapter producing a self-contained HTML page.
#[derive(Debug, Clone)]
pub struct HtmlRenderer {
    locale: String,
}

impl Default for HtmlRenderer {
    fn default() -> Self {
        Self::new(None)
    }
}

impl HtmlRenderer {
    /// Construct a new HTML renderer.
    ///
    /// The optional `locale` value sets the resulting page's `<html lang>`
    /// attribute. When `None` the renderer defaults to `en` to keep the page
    /// valid for assistive tooling.
    #[must_use]
    pub fn new(locale: Option<&str>) -> Self {
        Self {
            locale: locale.unwrap_or("en").to_owned(),
        }
    }
}

impl GraphRenderer for HtmlRenderer {
    fn render(&self, view: &GraphView, sink: &mut dyn Write) -> Result<(), GraphRenderError> {
        let positions = layout_positions(view);
        let title = localized(keys::GRAPH_HTML_TITLE);
        let heading = localized(keys::GRAPH_HTML_HEADING);
        let description = format!(
            "{} ({}, {})",
            localized(keys::GRAPH_HTML_DESCRIPTION),
            view.nodes.len(),
            view.edges.len()
        );
        let outline_summary = localized(keys::GRAPH_HTML_OUTLINE_SUMMARY);
        let noscript_notice = localized(keys::GRAPH_HTML_NOSCRIPT_NOTICE);

        writeln!(sink, "<!doctype html>")?;
        writeln!(sink, "<html lang=\"{}\">", escape_attr(&self.locale))?;
        writeln!(sink, "<head>")?;
        writeln!(sink, "  <meta charset=\"utf-8\">")?;
        writeln!(
            sink,
            "  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">"
        )?;
        writeln!(sink, "  <title>{}</title>", escape_text(&title))?;
        write_inline_style(sink)?;
        writeln!(sink, "</head>")?;
        writeln!(sink, "<body>")?;
        writeln!(sink, "  <h1>{}</h1>", escape_text(&heading))?;
        write_svg(
            sink,
            view,
            &positions,
            SvgHeader {
                title: &title,
                description: &description,
            },
        )?;
        write_outline(sink, view, &outline_summary)?;
        write_noscript(sink, view, &noscript_notice)?;
        writeln!(sink, "</body>")?;
        writeln!(sink, "</html>")?;
        Ok(())
    }
}

fn localized(key: &'static str) -> String {
    localization::message(key).to_string()
}

fn write_inline_style(sink: &mut dyn Write) -> Result<(), GraphRenderError> {
    // Inline-only styles: no external resources are referenced.
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

#[derive(Debug, Clone, Copy)]
struct Position {
    x: i32,
    y: i32,
}

fn layout_positions(view: &GraphView) -> BTreeMap<&Utf8Path, Position> {
    // Topological depth: sources at depth 0, then `depth(input) + 1` per node.
    // Cycles are rejected upstream by `BuildGraph::from_manifest`.
    let predecessors: BTreeMap<&Utf8Path, Vec<&Utf8Path>> = collect_predecessors(&view.edges);
    let mut depths: BTreeMap<&Utf8Path, i32> = BTreeMap::new();
    for node in &view.nodes {
        compute_depth(node.path.as_path(), &predecessors, &mut depths);
    }
    let mut by_depth: BTreeMap<i32, Vec<&Utf8Path>> = BTreeMap::new();
    for node in &view.nodes {
        let depth = *depths.get(node.path.as_path()).unwrap_or(&0);
        by_depth.entry(depth).or_default().push(node.path.as_path());
    }
    let mut positions = BTreeMap::new();
    for (depth, paths) in &by_depth {
        for (row, path) in paths.iter().enumerate() {
            positions.insert(
                *path,
                Position {
                    x: MARGIN + depth * COL_WIDTH,
                    // `row * ROW_HEIGHT` fits in i32 for any realistic graph.
                    y: MARGIN + i32::try_from(row).unwrap_or(i32::MAX) * ROW_HEIGHT,
                },
            );
        }
    }
    positions
}

fn collect_predecessors(edges: &[EdgeView]) -> BTreeMap<&Utf8Path, Vec<&Utf8Path>> {
    let mut preds: BTreeMap<&Utf8Path, Vec<&Utf8Path>> = BTreeMap::new();
    for edge in edges {
        preds
            .entry(edge.to.as_path())
            .or_default()
            .push(edge.from.as_path());
    }
    preds
}

fn compute_depth<'a>(
    path: &'a Utf8Path,
    predecessors: &BTreeMap<&'a Utf8Path, Vec<&'a Utf8Path>>,
    cache: &mut BTreeMap<&'a Utf8Path, i32>,
) -> i32 {
    if let Some(depth) = cache.get(path) {
        return *depth;
    }
    // Insert 0 first to break any unexpected cycle defensively.
    cache.insert(path, 0);
    let depth = predecessors.get(path).map_or(0, |preds| {
        preds
            .iter()
            .map(|pred| compute_depth(pred, predecessors, cache))
            .max()
            .map_or(0, |m| m.saturating_add(1))
    });
    cache.insert(path, depth);
    depth
}

#[derive(Clone, Copy)]
struct SvgHeader<'a> {
    title: &'a str,
    description: &'a str,
}

fn write_svg(
    sink: &mut dyn Write,
    view: &GraphView,
    positions: &BTreeMap<&Utf8Path, Position>,
    header: SvgHeader<'_>,
) -> Result<(), GraphRenderError> {
    let title = header.title;
    let description = header.description;
    let (width, height) = canvas_extent(positions);
    writeln!(
        sink,
        "  <svg role=\"img\" aria-labelledby=\"svg-title svg-desc\" viewBox=\"0 0 {width} {height}\" width=\"{width}\" height=\"{height}\" xmlns=\"http://www.w3.org/2000/svg\">"
    )?;
    writeln!(
        sink,
        "    <title id=\"svg-title\">{}</title>",
        escape_text(title)
    )?;
    writeln!(
        sink,
        "    <desc id=\"svg-desc\">{}</desc>",
        escape_text(description)
    )?;
    for edge in &view.edges {
        write_svg_edge(sink, edge, positions)?;
    }
    for node in &view.nodes {
        write_svg_node(sink, node, positions)?;
    }
    writeln!(sink, "  </svg>")?;
    Ok(())
}

fn canvas_extent(positions: &BTreeMap<&Utf8Path, Position>) -> (i32, i32) {
    let max_x = positions
        .values()
        .map(|p| p.x + NODE_WIDTH)
        .max()
        .unwrap_or(NODE_WIDTH);
    let max_y = positions
        .values()
        .map(|p| p.y + NODE_HEIGHT)
        .max()
        .unwrap_or(NODE_HEIGHT);
    (max_x + MARGIN, max_y + MARGIN)
}

fn write_svg_node(
    sink: &mut dyn Write,
    node: &NodeView,
    positions: &BTreeMap<&Utf8Path, Position>,
) -> Result<(), GraphRenderError> {
    let pos = positions
        .get(node.path.as_path())
        .copied()
        .unwrap_or(Position {
            x: MARGIN,
            y: MARGIN,
        });
    let class = match node.kind {
        NodeKind::Source => "node source",
        NodeKind::Target { phony: true, .. } => "node phony",
        NodeKind::Target {
            phony: false,
            always: true,
        } => "node always",
        NodeKind::Target {
            phony: false,
            always: false,
        } => "node target",
    };
    let label = node.path.as_str();
    writeln!(
        sink,
        "    <g class=\"{class}\" aria-label=\"{}\">",
        escape_attr(label)
    )?;
    writeln!(
        sink,
        "      <rect x=\"{}\" y=\"{}\" width=\"{NODE_WIDTH}\" height=\"{NODE_HEIGHT}\"></rect>",
        pos.x, pos.y
    )?;
    let text_x = pos.x + 8;
    let text_y = pos.y + 18;
    writeln!(
        sink,
        "      <text x=\"{text_x}\" y=\"{text_y}\">{}</text>",
        escape_text(label)
    )?;
    if let Some(desc) = node.description.as_deref().filter(|d| !d.is_empty()) {
        let desc_y = pos.y + 34;
        writeln!(
            sink,
            "      <text x=\"{text_x}\" y=\"{desc_y}\" font-size=\"10\" fill=\"#444\">{}</text>",
            escape_text(desc)
        )?;
    }
    writeln!(sink, "    </g>")?;
    Ok(())
}

fn write_svg_edge(
    sink: &mut dyn Write,
    edge: &EdgeView,
    positions: &BTreeMap<&Utf8Path, Position>,
) -> Result<(), GraphRenderError> {
    let Some(&from) = positions.get(edge.from.as_path()) else {
        return Ok(());
    };
    let Some(&to) = positions.get(edge.to.as_path()) else {
        return Ok(());
    };
    let x1 = from.x + NODE_WIDTH;
    let y1 = from.y + NODE_HEIGHT_HALF;
    let x2 = to.x;
    let y2 = to.y + NODE_HEIGHT_HALF;
    let class = match edge.class {
        EdgeClass::Explicit => "edge",
        EdgeClass::ImplicitDep => "edge implicit-dep",
        EdgeClass::ImplicitOutput => "edge implicit-output",
        EdgeClass::OrderOnly => "edge order-only",
    };
    let title = format!("{} → {}", edge.from, edge.to);
    // `x2 >= x1` in a left-to-right layered layout, so an arithmetic shift
    // is equivalent to division by two while avoiding the workspace ban on
    // `/` for integers.
    let midpoint_x = x1 + ((x2 - x1) >> 1);
    writeln!(sink, "    <g class=\"{class}\">")?;
    writeln!(sink, "      <title>{}</title>", escape_text(&title))?;
    writeln!(
        sink,
        "      <path d=\"M{x1} {y1} L{midpoint_x} {y1} L{midpoint_x} {y2} L{x2} {y2}\"></path>"
    )?;
    writeln!(sink, "    </g>")?;
    Ok(())
}

fn write_outline(
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

fn write_noscript(
    sink: &mut dyn Write,
    view: &GraphView,
    notice: &str,
) -> Result<(), GraphRenderError> {
    writeln!(sink, "  <noscript>")?;
    writeln!(sink, "    <p>{}</p>", escape_text(notice))?;
    writeln!(sink, "    <pre><code>")?;
    let mut dot_buf: Vec<u8> = Vec::new();
    DotRenderer::new().render(view, &mut dot_buf)?;
    let dot = String::from_utf8(dot_buf).unwrap_or_default();
    write!(sink, "{}", escape_text(&dot))?;
    writeln!(sink, "</code></pre>")?;
    writeln!(sink, "  </noscript>")?;
    Ok(())
}

fn escape_html(input: &str, attr: bool) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            '\'' if attr => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

fn escape_text(input: &str) -> String {
    escape_html(input, false)
}

fn escape_attr(input: &str) -> String {
    escape_html(input, true)
}

#[cfg(test)]
#[path = "render_html_tests.rs"]
mod tests;
