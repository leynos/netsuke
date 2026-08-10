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
//! use netsuke::ast::{Recipe, DependencyOrder};
//! use netsuke::ir::{BuildEdge, BuildGraph};
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

use crate::ast::DependencyOrder;
use crate::hex;
use crate::ir::{BuildEdge, BuildGraph};
use crate::localization::{self, keys};
use crate::ninja_gen::{NinjaGenError, join, path_key};
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

/// One generated dyndep sidecar file inside a [`GeneratedNinja`] bundle.
///
/// `relative_path` is relative to the effective Ninja working directory and
/// matches the path the main build file references. `content` is the full
/// Ninja-syntax dyndep document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedDyndep {
    relative_path: Utf8PathBuf,
    content: String,
}

impl GeneratedDyndep {
    /// Borrow the sidecar path relative to the effective Ninja working
    /// directory.
    #[must_use]
    pub fn relative_path(&self) -> &Utf8PathBuf {
        &self.relative_path
    }

    /// Borrow the dyndep document content to materialize.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }
}

/// The complete generated Ninja artefact: the main build file text plus every
/// dyndep sidecar required to load and execute it.
///
/// All paths are relative to the effective Ninja working directory. Do not
/// invoke Ninja on [`GeneratedNinja::build_file`] until every sidecar in
/// [`GeneratedNinja::dyndep_files`] has been materialized beside it.
#[derive(Debug, Clone)]
pub struct GeneratedNinja {
    build_file: String,
    dyndep_files: Vec<GeneratedDyndep>,
}

impl GeneratedNinja {
    /// Borrow the main Ninja build file text.
    #[must_use]
    pub fn build_file(&self) -> &str {
        &self.build_file
    }

    /// Borrow the dyndep sidecars required by `build_file`.
    #[must_use]
    pub fn dyndep_files(&self) -> &[GeneratedDyndep] {
        &self.dyndep_files
    }

    /// Consume the bundle, returning the main file text and its sidecars.
    #[must_use]
    pub fn into_parts(self) -> (String, Vec<GeneratedDyndep>) {
        (self.build_file, self.dyndep_files)
    }
}

#[cfg(test)]
impl GeneratedDyndep {
    /// Build a sidecar fixture for tests that must construct bundles from
    /// scratch rather than through .
    #[must_use]
    pub(crate) fn fixture(relative_path: Utf8PathBuf, content: String) -> Self {
        Self {
            relative_path,
            content,
        }
    }
}

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
    reject_reserved_paths(graph)?;
    let serial_present = graph_requires_dyndep(graph);

    let mut out = String::new();
    if serial_present {
        writeln!(out, "ninja_required_version = 1.10\n").expect("write to String cannot fail");
    }

    let mut actions: Vec<_> = graph.actions.iter().collect();
    actions.sort_by_key(|(id, _)| *id);
    for (id, action) in actions {
        use crate::ninja_gen::NamedAction;
        writeln!(out, "{}", NamedAction { id, action }).expect("write to String cannot fail");
    }

    let mut edges: Vec<_> = graph.targets.values().collect();
    edges.sort_by_key(|a| path_key(&a.explicit_outputs));
    let mut seen = HashSet::new();
    let mut dyndep_files: Vec<GeneratedDyndep> = Vec::new();
    let mut staged_sidecars: HashSet<Utf8PathBuf> = HashSet::new();

    for edge in edges {
        let key = path_key(&edge.explicit_outputs);
        if !seen.insert(key.clone()) {
            continue;
        }
        let action =
            graph
                .actions
                .get(&edge.action_id)
                .ok_or_else(|| NinjaGenError::MissingAction {
                    id: edge.action_id.clone(),
                    message: localization::message(keys::NINJA_GEN_MISSING_ACTION)
                        .with_arg("id", &edge.action_id),
                })?;

        let requires_gates =
            edge.dependency_order == DependencyOrder::Serial && edge.implicit_deps.len() > 1;
        if requires_gates {
            let mut added = Vec::new();
            render_serial_block(
                edge,
                &mut out,
                &mut dyndep_files,
                &mut staged_sidecars,
                &mut added,
            )
            .expect("write to String cannot fail");
            let mut aggregate = edge.clone();
            aggregate.implicit_deps = added;
            aggregate.dependency_order = DependencyOrder::Parallel;
            writeln!(
                out,
                "{}",
                crate::ninja_gen::DisplayEdge {
                    edge: &aggregate,
                    action_restat: action.restat,
                }
            )
            .expect("write to String cannot fail");
        } else {
            writeln!(
                out,
                "{}",
                crate::ninja_gen::DisplayEdge {
                    edge,
                    action_restat: action.restat,
                }
            )
            .expect("write to String cannot fail");
        }
    }

    if !graph.default_targets.is_empty() {
        let mut defs = graph.default_targets.clone();
        defs.sort();
        writeln!(out, "default {}", join(&defs)).expect("write to String cannot fail");
    }

    Ok(GeneratedNinja {
        build_file: out,
        dyndep_files,
    })
}

/// Emit the staged gates and sidecar-producing phony edges for one serial edge,
/// collecting each sidecar into the bundle and returning the gate paths in
/// dependency order.
fn render_serial_block(
    edge: &BuildEdge,
    out: &mut String,
    dyndep_files: &mut Vec<GeneratedDyndep>,
    staged_sidecars: &mut HashSet<Utf8PathBuf>,
    gate_paths: &mut Vec<Utf8PathBuf>,
) -> std::fmt::Result {
    use crate::ninja_gen::escape_ninja_path;

    let parent = parent_identity(edge);
    let mut previous_gate: Option<Utf8PathBuf> = None;
    for (index, dep) in edge.implicit_deps.iter().enumerate() {
        let gate = parent.join(format!("{index:03}"));
        let content = sidecar_content(&gate, dep);
        let digest = sidecar_digest(&content);
        let sidecar = Utf8PathBuf::from(format!("{DYNDEP_NAMESPACE}/{digest}.dd"));

        let sidecar_escaped = escape_ninja_path(sidecar.as_str());
        let gate_escaped = escape_ninja_path(gate.as_str());

        // The phony edge that produces (but never rebuilds) the sidecar file.
        // Starting at the second stage it depends on the previous gate, which
        // prevents Ninja from revealing the next sidecar early.
        match &previous_gate {
            None => writeln!(out, "build {sidecar_escaped}: phony")?,
            Some(prev) => {
                let prev_escaped = escape_ninja_path(prev.as_str());
                writeln!(out, "build {sidecar_escaped}: phony {prev_escaped}")?;
            }
        }
        // The gate edge: order-only depends on the sidecar and declares it as
        // its dyndep file so Ninja loads the real dependency from the sidecar.
        writeln!(out, "build {gate_escaped}: phony || {sidecar_escaped}")?;
        writeln!(out, "  dyndep = {sidecar_escaped}")?;
        writeln!(out)?;

        if staged_sidecars.insert(sidecar.clone()) {
            dyndep_files.push(GeneratedDyndep {
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
fn sidecar_content(gate: &Utf8PathBuf, dep: &Utf8PathBuf) -> String {
    use crate::ninja_gen::escape_ninja_path;
    let gate_escaped = escape_ninja_path(gate.as_str());
    let dep_escaped = escape_ninja_path(dep.as_str());
    format!("ninja_dyndep_version = 1\nbuild {gate_escaped}: dyndep | {dep_escaped}\n")
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
            if as_str == SERIAL_NAMESPACE
                || as_str == DYNDEP_NAMESPACE
                || as_str.starts_with(&format!("{SERIAL_NAMESPACE}/"))
                || as_str.starts_with(&format!("{DYNDEP_NAMESPACE}/"))
            {
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

/// Whether the graph contains an edge that needs staged dyndep gates.
fn graph_requires_dyndep(graph: &BuildGraph) -> bool {
    graph.targets.values().any(|edge| {
        edge.dependency_order == DependencyOrder::Serial && edge.implicit_deps.len() > 1
    })
}

#[cfg(test)]
#[path = "dyndep_tests.rs"]
mod tests;
