//! Build graph data structures for the intermediate representation (IR).
//!
//! Defines the core IR types: [`BuildGraph`] (the complete static build
//! graph), [`BuildEdge`] (a single target with its `inputs`, `implicit_deps`,
//! and `order_only_deps`), and [`Action`] (a deduplicated build rule plus
//! recipe).  `implicit_deps` mirror Ninja's `|` syntax — they trigger
//! rebuilds but are not passed to `{{ ins }}`. Consumed by [`crate::ninja_gen`]
//! for Ninja file emission and by [`super::cycle`] for cycle detection.

#[cfg(not(kani))]
use crate::localization::{self, keys};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;
#[cfg(not(kani))]
use std::collections::HashMap;

use crate::ast::Recipe;

#[cfg(kani)]
#[path = "graph_kani_map.rs"]
mod kani_map;

#[cfg(kani)]
pub use kani_map::{IrHashMap, IrVec};

/// Map used by the IR graph.
#[cfg(not(kani))]
pub type IrHashMap<K, V> = HashMap<K, V>;

/// Arena used to own canonical build edges.
#[cfg(kani)]
pub type EdgeArena<T> = IrVec<T>;

/// Arena used to own canonical build edges.
#[cfg(not(kani))]
pub type EdgeArena<T> = Vec<T>;

/// The complete, static build graph.
#[derive(Debug, Default, Clone)]
pub struct BuildGraph {
    /// All unique actions in the build keyed by a stable hash.
    pub actions: IrHashMap<String, Action>,
    /// Canonical build edges, each owned exactly once.
    edges: EdgeArena<BuildEdge>,
    /// Output aliases indexed to their canonical producing edge.
    targets: IrHashMap<Utf8PathBuf, EdgeId>,
    /// Targets built when no explicit target is requested.
    pub default_targets: Vec<Utf8PathBuf>,
}

/// Identifies one canonical [`BuildEdge`] in a [`BuildGraph`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeId(usize);

impl BuildGraph {
    /// Store `edge` once when every output alias is unique before mutation.
    ///
    /// # Errors
    ///
    /// Returns [`IrGenError::DuplicateOutput`] without changing the arena or
    /// output index when an explicit or implicit output collides with another
    /// alias on the edge or in this graph.
    #[cfg(not(kani))]
    pub fn insert_edge(&mut self, edge: BuildEdge) -> Result<EdgeId, IrGenError> {
        if let Some(output) = self.duplicate_output(&edge) {
            return Err(IrGenError::DuplicateOutput {
                message: localization::message(keys::IR_DUPLICATE_OUTPUTS)
                    .with_arg("outputs", output.as_str()),
                outputs: vec![output.as_str().to_owned()],
            });
        }
        Ok(self.insert_canonical_edge(edge))
    }

    /// Store `edge` once in Kani's bounded graph model.
    #[cfg(kani)]
    pub fn insert_edge(&mut self, edge: BuildEdge) -> EdgeId {
        self.insert_canonical_edge(edge)
    }

    /// Store `edge` through the fallible canonical insertion API.
    ///
    /// # Errors
    ///
    /// Returns [`IrGenError::DuplicateOutput`] without changing the arena or
    /// output index when an explicit or implicit output collides with another
    /// alias on the edge or in this graph.
    #[cfg(not(kani))]
    pub fn try_insert_edge(&mut self, edge: BuildEdge) -> Result<EdgeId, IrGenError> {
        self.insert_edge(edge)
    }

    /// Store one canonical edge and index its output aliases.
    fn insert_canonical_edge(&mut self, edge: BuildEdge) -> EdgeId {
        let edge_id = EdgeId(self.edges.len());
        self.edges.push(edge);
        self.index_output_aliases(edge_id);
        edge_id
    }

    /// Index every output alias owned by the canonical edge at `edge_id`.
    #[cfg(not(kani))]
    fn index_output_aliases(&mut self, edge_id: EdgeId) {
        if let Some(stored_edge) = self.edges.get(edge_id.0) {
            for output in stored_edge
                .explicit_outputs
                .iter()
                .chain(&stored_edge.implicit_outputs)
            {
                self.targets.insert(output.clone(), edge_id);
            }
        }
    }

    /// Index every bounded output alias owned by the canonical edge at `edge_id`.
    #[cfg(kani)]
    fn index_output_aliases(&mut self, edge_id: EdgeId) {
        if let Some(stored_edge) = self.edges.get(edge_id.0) {
            for output in &stored_edge.explicit_outputs {
                self.targets.insert(output.clone(), edge_id);
            }
            for output in &stored_edge.implicit_outputs {
                self.targets.insert(output.clone(), edge_id);
            }
        }
    }

    /// Return the number of canonical build edges in the arena.
    #[must_use]
    pub const fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Return the number of output aliases in the index.
    #[must_use]
    pub fn output_count(&self) -> usize {
        self.targets.len()
    }

    /// Return the bounded output index for Kani lowering harnesses.
    #[cfg(kani)]
    pub(super) fn output_index(&self) -> &IrHashMap<Utf8PathBuf, EdgeId> {
        &self.targets
    }

    /// Return the canonical edge identity for `output`, when indexed.
    #[cfg(not(kani))]
    #[must_use]
    pub fn edge_id_for_output(&self, output: &Utf8Path) -> Option<EdgeId> {
        self.targets.get(output).copied()
    }

    /// Return the canonical edge identity for `output` in Kani's path model.
    #[cfg(kani)]
    #[must_use]
    pub fn edge_id_for_output(&self, output: &Utf8Path) -> Option<EdgeId> {
        self.targets
            .get_key_value_path(output)
            .map(|(_, edge_id)| *edge_id)
    }

    /// Replace the edge for `output` while preserving its output aliases.
    ///
    /// Returns `false` when `output` is absent, the arena is inconsistent, or
    /// `replacement` would change either explicit or implicit output aliases.
    pub fn replace_edge_for_output(&mut self, output: &Utf8Path, replacement: BuildEdge) -> bool {
        let Some(edge_id) = self.edge_id_for_output(output) else {
            return false;
        };
        let Some(existing) = self.edges.get_mut(edge_id.0) else {
            return false;
        };
        if existing.explicit_outputs != replacement.explicit_outputs
            || existing.implicit_outputs != replacement.implicit_outputs
        {
            return false;
        }
        *existing = replacement;
        true
    }

    /// Resolve `output` to its canonical stored key and producing edge.
    #[cfg(not(kani))]
    #[must_use]
    pub fn target_for_output(&self, output: &Utf8Path) -> Option<(&Utf8Path, &BuildEdge)> {
        self.targets
            .get_key_value(output)
            .and_then(|(stored_output, edge_id)| {
                self.edges
                    .get(edge_id.0)
                    .map(|edge| (stored_output.as_path(), edge))
            })
    }

    /// Resolve `output` through Kani's bounded path-key model.
    #[cfg(kani)]
    #[must_use]
    pub fn target_for_output(&self, output: &Utf8Path) -> Option<(&Utf8Path, &BuildEdge)> {
        self.targets
            .get_key_value_path(output)
            .and_then(|(stored_output, edge_id)| {
                self.edges
                    .get(edge_id.0)
                    .map(|edge| (stored_output.as_path(), edge))
            })
    }

    /// Iterate the graph's canonical build edges in arena insertion order.
    pub fn edges(&self) -> impl Iterator<Item = &BuildEdge> {
        self.edges.iter()
    }

    /// Iterate every explicit or implicit output alias in the output index.
    pub fn output_paths(&self) -> impl Iterator<Item = &Utf8PathBuf> {
        self.targets.keys()
    }

    /// Return the first output alias that would duplicate an alias in `edge`.
    #[cfg(not(kani))]
    fn duplicate_output<'edge>(&self, edge: &'edge BuildEdge) -> Option<&'edge Utf8PathBuf> {
        let mut seen = Vec::new();
        for output in edge.explicit_outputs.iter().chain(&edge.implicit_outputs) {
            if self.targets.contains_key(output) || seen.contains(&output) {
                return Some(output);
            }
            seen.push(output);
        }
        None
    }
}

/// Dependency scheduling policy carried by the domain build graph.
///
/// This IR type deliberately has no serialization responsibility. Manifest
/// spelling and Serde behaviour belong to [`crate::ast::DependencyOrder`] and
/// are converted explicitly while lowering the manifest into the graph.
///
/// ```compile_fail
/// use netsuke::ir::DependencyOrder;
///
/// // Serialize the manifest AST policy, not its lowered domain counterpart.
/// serde_json::to_string(&DependencyOrder::Parallel)?;
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DependencyOrder {
    /// Dependencies may run in any order allowed by the build scheduler.
    #[default]
    Parallel,
    /// Dependencies run in declaration order, one after another.
    Serial,
}

/// A reusable command analogous to a Ninja rule.

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Action {
    /// Recipe invoked by Ninja when this action is referenced.
    pub recipe: Recipe,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional human-readable description used by Ninja's `description` flag.
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional depfile path provided to Ninja's `depfile` attribute.
    pub depfile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional dependency format (`deps`) such as `gcc`.
    pub deps_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional Ninja pool to throttle concurrent execution.
    pub pool: Option<String>,
    /// Flag mirroring Ninja's `restat` behaviour for timestamp stability.
    pub restat: bool,
}

/// A single build statement connecting inputs to outputs.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildEdge {
    /// Identifier of the [`Action`] used for this edge.
    pub action_id: String,
    /// Explicit inputs that trigger a rebuild when changed.
    pub inputs: Vec<Utf8PathBuf>,
    /// Implicit dependencies that trigger a rebuild without entering recipes.
    pub implicit_deps: Vec<Utf8PathBuf>,
    /// Ordering policy applied to `implicit_deps` when they come from a
    /// manifest `deps` list. Parallel is the default; serial dependencies are
    /// lowered into staged Ninja dyndep gates by the generator.
    pub dependency_order: DependencyOrder,
    /// Outputs explicitly generated by the command.
    pub explicit_outputs: Vec<Utf8PathBuf>,
    /// Outputs implicitly generated by the command (Ninja `|`).
    pub implicit_outputs: Vec<Utf8PathBuf>,
    /// Order-only dependencies that do not trigger rebuilds (Ninja `||`).
    pub order_only_deps: Vec<Utf8PathBuf>,
    /// Output does not correspond to a real file.
    pub phony: bool,
    /// Run the command on every invocation regardless of timestamps.
    pub always: bool,
}

#[path = "graph_error.rs"]
mod graph_error;

pub use graph_error::IrGenError;
