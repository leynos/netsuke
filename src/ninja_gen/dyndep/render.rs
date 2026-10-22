//! Render user graph edges and staged gates in a generated Ninja bundle.
//!
//! This module owns the dyndep edge-lowering details and is called only by
//! bundle generation. Keep the ordering and output policy here; the parent
//! module owns the bundle API and reserved-path validation.

use super::{DYNDEP_NAMESPACE, GeneratedDyndep, SERIAL_NAMESPACE};
use crate::hex;
use crate::ir::{BuildEdge, BuildGraph};
use crate::ninja_gen::edge_metadata;
use crate::ninja_gen::escape::NinjaValue;
use crate::ninja_gen::{
    DisplayEdge, NinjaGenError, edge_requires_gates, ordered_edges, validated_ninja_path,
};
use camino::Utf8PathBuf;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt::Write as _;

/// Schema tag incorporated into every parent (gate-set) identity.
const PARENT_SCHEMA: &str = "netsuke-serial-v1";
/// Format tag incorporated into every dyndep filename digest.
const DYNDEP_SCHEMA: &str = "netsuke-dyndep-v1";
/// Filename prefix distinguishing a stage's relay phony from its gate.
const RELAY_PREFIX: &str = "relay-";

/// Mutable staging state shared while lowering one serial edge.
#[derive(Default)]
struct SerialStages {
    /// Dyndep sidecars collected while lowering serial edges.
    dyndep_files: Vec<GeneratedDyndep>,
    /// Sidecar paths already staged, deduplicating shared dependencies.
    staged_sidecars: HashSet<Utf8PathBuf>,
}

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
}

/// Emit the staged relays, gates, and sidecar-producing phony edges for one
/// serial edge, collecting each sidecar into the bundle and returning the gate
/// paths in dependency order.
///
/// Each stage emits three statements — a relay carrying the real dependency, a
/// phony producing the sidecar, and the gate — so a serial edge with `N`
/// implicit dependencies contributes `1 + 3N` statements to the file.
fn render_serial_block(
    edge: &BuildEdge,
    out: &mut String,
    stages: &mut SerialStages,
    gate_paths: &mut Vec<Utf8PathBuf>,
) -> Result<(), NinjaGenError> {
    let parent = parent_identity(edge);
    for (index, dep) in edge.implicit_deps.iter().enumerate() {
        let gate = parent.join(format!("{index:03}"));
        let relay = parent.join(format!("{RELAY_PREFIX}{index:03}"));
        let content = sidecar_content(&gate, &relay)?;
        let digest = sidecar_digest(&content);
        let sidecar = Utf8PathBuf::from(format!("{DYNDEP_NAMESPACE}/{digest}.dd"));

        let sidecar_escaped = validated_ninja_path(sidecar.as_str())?;
        let gate_escaped = validated_ninja_path(gate.as_str())?;
        let relay_escaped = validated_ninja_path(relay.as_str())?;
        let dep_escaped = validated_ninja_path(dep.as_str())?;

        // The relay carries the real dependency and is what the sidecar
        // reveals. It is emitted before the gate so the revealed name is
        // already declared where the file is read top to bottom.
        writeln!(out, "build {relay_escaped}: phony | {dep_escaped}")?;
        // The phony edge that produces (but never rebuilds) the sidecar file.
        // Starting at the second stage it depends on the previous gate, which
        // prevents Ninja from revealing the next sidecar early.
        match gate_paths.last() {
            None => writeln!(out, "build {sidecar_escaped}: phony")?,
            Some(prev) => {
                let prev_escaped = validated_ninja_path(prev.as_str())?;
                writeln!(out, "build {sidecar_escaped}: phony {prev_escaped}")?;
            }
        }
        // The gate edge: order-only depends on the sidecar and declares it as
        // its dyndep file so Ninja loads the revealed relay from the sidecar.
        writeln!(out, "build {gate_escaped}: phony || {sidecar_escaped}")?;
        writeln!(out, "  dyndep = {sidecar_escaped}")?;
        writeln!(out)?;

        if stages.staged_sidecars.insert(sidecar.clone()) {
            stages.dyndep_files.push(GeneratedDyndep {
                relative_path: sidecar,
                content,
            });
        }
        gate_paths.push(gate);
    }
    Ok(())
}

/// Derive the stable parent (gate-set) identity for a serial edge.
///
/// The identity is a SHA-256 over the edge canonical output identity and an
/// explicit schema tag, so renaming the output or changing the staging format
/// yields a fresh namespace without colliding with other serial edges.
fn parent_identity(edge: &BuildEdge) -> Utf8PathBuf {
    let canonical = edge
        .explicit_outputs
        .iter()
        .map(|path| path.as_str())
        .collect::<Vec<_>>()
        .join("\u{0}");
    let mut hasher = Sha256::new();
    hasher.update(PARENT_SCHEMA.as_bytes());
    hasher.update(b"\0");
    hasher.update(canonical.as_bytes());
    let digest = hex::to_lower_hex(&hasher.finalize());
    Utf8PathBuf::from(format!("{SERIAL_NAMESPACE}/{digest}"))
}

/// Render the dyndep document revealing one stage's relay phony.
///
/// The revealed path is the relay rather than the real dependency so that
/// Ninja's `AddOutEdge` targets a node with no other consumer; see the module
/// documentation for the upstream crash this avoids.
fn sidecar_content(gate: &Utf8PathBuf, relay: &Utf8PathBuf) -> Result<String, NinjaGenError> {
    let gate_escaped = validated_ninja_path(gate.as_str())?;
    let relay_escaped = validated_ninja_path(relay.as_str())?;
    Ok(format!(
        "ninja_dyndep_version = 1\nbuild {gate_escaped}: dyndep | {relay_escaped}\n"
    ))
}

/// Content-address a sidecar by its complete bytes and a format tag.
fn sidecar_digest(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DYNDEP_SCHEMA.as_bytes());
    hasher.update(b"\0");
    hasher.update(content.as_bytes());
    hex::to_lower_hex(&hasher.finalize())
}
