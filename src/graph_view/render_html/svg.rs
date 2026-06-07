//! SVG rendering for nodes and edges in the HTML graph view.
//!
//! Edges are drawn before nodes so that the rectangular node bodies overlay
//! the orthogonal connectors at their endpoints. Every node carries an
//! `aria-label` and every edge a `<title>` so the SVG is navigable by screen
//! readers despite the layered visual layout.

use std::collections::BTreeMap;
use std::io::Write;

use camino::Utf8Path;

use crate::graph_view::render::GraphRenderError;
use crate::graph_view::{EdgeClass, EdgeView, GraphView, NodeKind, NodeView};

use super::escape::{escape_attr, escape_text};
use super::layout::{MARGIN, NODE_HEIGHT, NODE_HEIGHT_HALF, NODE_WIDTH, Position};

#[derive(Clone, Copy)]
pub(super) struct SvgHeader<'a> {
    pub title: &'a str,
    pub description: &'a str,
}

pub(super) fn write_svg(
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
        // A phony target has no on-disk artefact, so the phony style takes
        // precedence even if `always` is also set.
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
