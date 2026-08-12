//! Staged Ninja dyndep lowering for serial dependency ordering.
//!
//! Netsuke keeps one top-level Ninja invocation. To make a `serial` `deps`
//! list execute in declaration order, this module lowers each serial edge
//! into a chain of phony gates plus content-addressed dyndep sidecars. Each
//! gate names one sidecar through Ninja `dyndep` binding; the sidecar
//! reveals exactly one real dependency. The next sidecar is not visible until
//! the preceding gate completes, so Ninja cannot schedule a later dependency
//! before an earlier one succeeds.
//!
//! The sidecars are immutable and content-addressed beneath `.netsuke/dyndep`,
//! and the gates live beneath `.netsuke/serial`. The runner materializes the
//! sidecars (and the main file) before invoking Ninja; string-only generation
//! rejects graphs that require a bundle.
//!
//! ```rust
//! use netsuke::ast::Recipe;
//! use netsuke::ir::{BuildEdge, BuildGraph, DependencyOrder};
//! use netsuke::ninja_gen::generate_bundle;
//! use camino::Utf8PathBuf;
//!
//! let action = netsuke::ir::Action {
//!     recipe: Recipe::Command { command: "echo done".into() },
//!     description: None,
//!     depfile: None,
//!     deps_format: None,
//!     pool: None,
//!     restat: false,
//! };
//! let mut graph = BuildGraph::default();
//! graph.actions.insert("a".into(), action);
//! graph.targets.insert(
//!     Utf8PathBuf::from("all"),
//!     BuildEdge {
//!         action_id: "a".into(),
//!         inputs: Vec::new(),
//!         implicit_deps: vec![
//!             Utf8PathBuf::from("check-fmt"),
//!             Utf8PathBuf::from("test"),
//!         ],
//!         dependency_order: DependencyOrder::Serial,
//!         explicit_outputs: vec![Utf8PathBuf::from("all")],
//!         implicit_outputs: Vec::new(),
//!         order_only_deps: Vec::new(),
//!         phony: false,
//!         always: false,
//!     },
//! );
//! let bundle = generate_bundle(&graph).expect("generate bundle");
//! assert!(bundle.build_file().contains("ninja_required_version = 1.10"));
//! assert_eq!(bundle.dyndep_files().len(), 2);
//! ```

#[path = "dyndep_bundle.rs"]
mod bundle;
pub use bundle::{GeneratedDyndep, GeneratedNinja};

use crate::hex;
use crate::ir::{BuildEdge, BuildGraph};
use crate::localization::{self, keys};
use crate::ninja_gen::{
    NinjaGenError, edge_requires_gates, graph_requires_dyndep, join, path_key,
    reject_unsupported_path_characters,
};
use camino::Utf8PathBuf;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt::Write as _;

/// Schema tag incorporated into every parent (gate-set) identity.
const PARENT_SCHEMA: &str = "netsuke-serial-v1";
/// Format tag incorporated into every dyndep filename digest.
const DYNDEP_SCHEMA: &str = "netsuke-dyndep-v1";
/// Reserved state namespace for serial gate paths.
const SERIAL_NAMESPACE: &str = ".netsuke/serial";
/// Reserved state namespace for dyndep sidecar files.
const DYNDEP_NAMESPACE: &str = ".netsuke/dyndep";

/// Generate a complete Ninja bundle for `graph`, materializing staged dyndep
/// sidecars for every multi-dependency serial edge.
///
/// Ordinary parallel graphs produce a bundle with an empty sidecar list and a
/// main file identical to [`crate::ninja_gen::generate`].
///
/// # Errors
///
/// Returns [`NinjaGenError::ReservedOutputPath`] when a user output or
/// dependency collides with the reserved `.netsuke/serial` or
/// `.netsuke/dyndep` namespace, and [`NinjaGenError::MissingAction`] when an
/// edge references an unknown action.
pub fn generate_bundle(graph: &BuildGraph) -> Result<GeneratedNinja, NinjaGenError> {
    generate_bundle_inner(graph)
}

fn generate_bundle_inner(graph: &BuildGraph) -> Result<GeneratedNinja, NinjaGenError> {
    reject_unsupported_path_characters(graph)?;
    reject_reserved_paths(graph)?;
    let serial_present = graph_requires_dyndep(graph);

    let mut out = String::new();
    if serial_present {
        writeln!(out, "ninja_required_version = 1.10\n")?;
    }

    let mut actions: Vec<_> = graph.actions.iter().collect();
    actions.sort_by_key(|(id, _)| *id);
    for (id, action) in actions {
        use crate::ninja_gen::NamedAction;
        write!(out, "{}", NamedAction { id, action })?;
    }

    let mut stages = SerialStages::default();
    render_edges(graph, &mut out, &mut stages)?;

    if !graph.default_targets.is_empty() {
        let mut defs = graph.default_targets.clone();
        defs.sort();
        writeln!(out, "default {}", join(&defs))?;
    }

    Ok(GeneratedNinja {
        build_file: out,
        dyndep_files: stages.dyndep_files,
    })
}

/// Render each distinct graph edge in stable output order.
fn render_edges(
    graph: &BuildGraph,
    out: &mut String,
    stages: &mut SerialStages,
) -> Result<(), NinjaGenError> {
    let mut edges: Vec<_> = graph.targets.values().collect();
    edges.sort_by_key(|a| path_key(&a.explicit_outputs));
    let mut seen: HashSet<String> = HashSet::new();

    for edge in edges {
        let key = path_key(&edge.explicit_outputs);
        if !seen.insert(key.clone()) {
            continue;
        }
        render_edge(graph, edge, out, stages)?;
    }
    Ok(())
}

/// Render one graph edge, applying staged dyndep lowering when required.
fn render_edge(
    graph: &BuildGraph,
    edge: &BuildEdge,
    out: &mut String,
    stages: &mut SerialStages,
) -> Result<(), NinjaGenError> {
    let action =
        graph
            .actions
            .get(&edge.action_id)
            .ok_or_else(|| NinjaGenError::MissingAction {
                id: edge.action_id.clone(),
                message: localization::message(keys::NINJA_GEN_MISSING_ACTION)
                    .with_arg("id", &edge.action_id),
            })?;

    if edge_requires_gates(edge) {
        return render_serial_edge(edge, action.restat, out, stages);
    }
    render_display_edge(edge, action.restat, &edge.implicit_deps, out)
}

/// Render one staged serial edge and replace its real dependencies with gates.
fn render_serial_edge(
    edge: &BuildEdge,
    action_restat: bool,
    out: &mut String,
    stages: &mut SerialStages,
) -> Result<(), NinjaGenError> {
    let mut gate_paths = Vec::new();
    render_serial_block(edge, out, stages, &mut gate_paths)?;
    render_display_edge(edge, action_restat, &gate_paths, out)
}

/// Render an edge using its already selected Ninja action metadata.
fn render_display_edge(
    edge: &BuildEdge,
    action_restat: bool,
    implicit_deps: &[Utf8PathBuf],
    out: &mut String,
) -> Result<(), NinjaGenError> {
    write!(
        out,
        "{}",
        crate::ninja_gen::DisplayEdge {
            edge,
            action_restat,
            implicit_deps,
        }
    )?;
    Ok(())
}

/// Mutable staging state shared while lowering one serial edge.
#[derive(Default)]
struct SerialStages {
    dyndep_files: Vec<GeneratedDyndep>,
    staged_sidecars: HashSet<Utf8PathBuf>,
}

/// Emit the staged gates and sidecar-producing phony edges for one serial edge,
/// collecting each sidecar into the bundle and returning the gate paths in
/// dependency order.
fn render_serial_block(
    edge: &BuildEdge,
    out: &mut String,
    stages: &mut SerialStages,
    gate_paths: &mut Vec<Utf8PathBuf>,
) -> Result<(), NinjaGenError> {
    use crate::ninja_gen::escape_ninja_path;

    let parent = parent_identity(edge);
    let mut previous_gate: Option<Utf8PathBuf> = None;
    for (index, dep) in edge.implicit_deps.iter().enumerate() {
        let gate = parent.join(format!("{index:03}"));
        let content = sidecar_content(&gate, dep)?;
        let digest = sidecar_digest(&content);
        let sidecar = Utf8PathBuf::from(format!("{DYNDEP_NAMESPACE}/{digest}.dd"));

        let sidecar_escaped = escape_ninja_path(sidecar.as_str())?;
        let gate_escaped = escape_ninja_path(gate.as_str())?;

        // The phony edge that produces (but never rebuilds) the sidecar file.
        // Starting at the second stage it depends on the previous gate, which
        // prevents Ninja from revealing the next sidecar early.
        match &previous_gate {
            None => writeln!(out, "build {sidecar_escaped}: phony")?,
            Some(prev) => {
                let prev_escaped = escape_ninja_path(prev.as_str())?;
                writeln!(out, "build {sidecar_escaped}: phony {prev_escaped}")?;
            }
        }
        // The gate edge: order-only depends on the sidecar and declares it as
        // its dyndep file so Ninja loads the real dependency from the sidecar.
        writeln!(out, "build {gate_escaped}: phony || {sidecar_escaped}")?;
        writeln!(out, "  dyndep = {sidecar_escaped}")?;
        writeln!(out)?;

        if stages.staged_sidecars.insert(sidecar.clone()) {
            stages.dyndep_files.push(GeneratedDyndep {
                relative_path: sidecar,
                content,
            });
        }
        gate_paths.push(gate.clone());
        previous_gate = Some(gate);
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
        .map(|p| p.as_str())
        .collect::<Vec<_>>()
        .join("\u{0}");
    let mut hasher = Sha256::new();
    hasher.update(PARENT_SCHEMA.as_bytes());
    hasher.update(b"\0");
    hasher.update(canonical.as_bytes());
    let digest = hex::to_lower_hex(&hasher.finalize());
    Utf8PathBuf::from(format!("{SERIAL_NAMESPACE}/{digest}"))
}

/// Render the dyndep document for one gate and its real dependency.
fn sidecar_content(gate: &Utf8PathBuf, dep: &Utf8PathBuf) -> Result<String, NinjaGenError> {
    use crate::ninja_gen::escape_ninja_path;
    let gate_escaped = escape_ninja_path(gate.as_str())?;
    let dep_escaped = escape_ninja_path(dep.as_str())?;
    Ok(format!(
        "ninja_dyndep_version = 1\nbuild {gate_escaped}: dyndep | {dep_escaped}\n"
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

/// Reject user outputs or dependencies that collide with the reserved
/// serial-ordering state namespace.
fn reject_reserved_paths(graph: &BuildGraph) -> Result<(), NinjaGenError> {
    for edge in graph.targets.values() {
        for path in edge
            .explicit_outputs
            .iter()
            .chain(&edge.implicit_outputs)
            .chain(&edge.inputs)
            .chain(&edge.implicit_deps)
            .chain(&edge.order_only_deps)
        {
            let as_str = path.as_str();
            let is_reserved = [SERIAL_NAMESPACE, DYNDEP_NAMESPACE]
                .iter()
                .any(|namespace| {
                    as_str == *namespace
                        || as_str
                            .strip_prefix(namespace)
                            .is_some_and(|suffix| suffix.starts_with('/'))
                });
            if is_reserved {
                return Err(NinjaGenError::ReservedOutputPath {
                    path: path.clone(),
                    message: localization::message(keys::NINJA_GEN_RESERVED_OUTPUT_PATH)
                        .with_arg("path", as_str),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "dyndep_tests.rs"]
mod tests;
