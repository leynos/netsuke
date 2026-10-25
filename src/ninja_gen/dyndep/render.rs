//! Render user graph edges and staged gates in a generated Ninja bundle.
//!
//! This module owns the dyndep edge-lowering details and is called only by
//! bundle generation. Keep the ordering and output policy here; the parent
//! module owns the bundle API and reserved-path validation.

use super::GeneratedDyndep;
use super::staging::{SerialStages, render_serial_block};
use crate::ir::{BuildEdge, BuildGraph};
use crate::ninja_gen::edge_metadata;
use crate::ninja_gen::escape::NinjaValue;
use crate::ninja_gen::{DisplayEdge, NinjaGenError, edge_requires_gates, ordered_edges};
use camino::Utf8PathBuf;
use std::fmt::Write as _;

/// Render each distinct graph edge in stable output order.
///
/// # Errors
///
/// Returns [`NinjaGenError`] when an edge references a missing action, its
/// progress description contains invalid Ninja metadata, or writing fails.
pub(super) fn render_edges(
    graph: &BuildGraph,
    out: &mut String,
    localizer: &dyn ortho_config::Localizer,
) -> Result<Vec<GeneratedDyndep>, NinjaGenError> {
    let mut renderer = EdgeRenderer {
        graph,
        out,
        stages: SerialStages::default(),
        localizer,
    };
    for edge in ordered_edges(graph.edges()) {
        renderer.render_edge(edge)?;
    }
    Ok(renderer.stages.dyndep_files)
}

/// Keep the bundle's shared edge-lowering dependencies together.
struct EdgeRenderer<'a> {
    /// Graph actions resolved while rendering its edges.
    graph: &'a BuildGraph,
    /// Main Ninja text receiving real edges and serial gates.
    out: &'a mut String,
    /// Dyndep sidecars collected while lowering serial edges.
    stages: SerialStages,
    /// Resolved locale reused for every inline progress label.
    localizer: &'a dyn ortho_config::Localizer,
}

impl EdgeRenderer<'_> {
    /// Render one graph edge, applying staged dyndep lowering when required.
    fn render_edge(&mut self, edge: &BuildEdge) -> Result<(), NinjaGenError> {
        let metadata = edge_metadata::for_edge(self.graph, edge, self.localizer)?;
        let rendered_action = RenderedAction {
            name: metadata.action_name,
            restat: metadata.restat,
            pool: metadata.pool,
        };
        let rendered_edge = RenderedEdge {
            action: rendered_action,
            description: metadata.description.as_ref(),
        };
        if edge_requires_gates(edge) {
            return self.render_serial_edge(edge, rendered_edge);
        }
        self.render_display_edge(edge, rendered_edge, &edge.implicit_deps)
    }

    /// Render one staged serial edge and replace its dependencies with gates.
    fn render_serial_edge(
        &mut self,
        edge: &BuildEdge,
        rendered_edge: RenderedEdge<'_>,
    ) -> Result<(), NinjaGenError> {
        let mut gate_paths = Vec::new();
        render_serial_block(edge, self.out, &mut self.stages, &mut gate_paths)?;
        self.render_display_edge(edge, rendered_edge, &gate_paths)
    }

    /// Render an edge using its already selected Ninja action metadata.
    fn render_display_edge(
        &mut self,
        edge: &BuildEdge,
        rendered_edge: RenderedEdge<'_>,
        implicit_deps: &[Utf8PathBuf],
    ) -> Result<(), NinjaGenError> {
        write!(
            self.out,
            "{}",
            DisplayEdge {
                edge,
                action_name: rendered_edge.action.name,
                action_restat: rendered_edge.action.restat,
                action_pool: rendered_edge.action.pool,
                implicit_deps,
                description: rendered_edge.description,
            }
        )?;
        Ok(())
    }
}

/// Hold the selected rule, dependencies and metadata for one rendered edge.
#[derive(Clone, Copy)]
struct RenderedEdge<'a> {
    /// Ninja action selected for the edge.
    action: RenderedAction<'a>,
    /// Optional safe inline progress description.
    description: Option<&'a NinjaValue>,
}

/// Hold the selected Ninja rule metadata while lowering a graph edge.
#[derive(Clone, Copy)]
struct RenderedAction<'a> {
    /// Name the Ninja rule selected for the edge.
    name: &'a str,
    /// Preserve whether Ninja should restat this edge after execution.
    restat: bool,
    /// Pool selected by the shared action, if any.
    pool: Option<&'a str>,
}
