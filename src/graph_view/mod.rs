//! Deterministic projection of the build graph for rendering adapters.
//!
//! [`GraphView`] is the domain port that every renderer (DOT, HTML, future
//! JSON) consumes. It is constructed once from [`BuildGraph`] and exposes a
//! canonical, fully sorted view that is invariant under `HashMap` iteration
//! order. Renderer adapters under this module read [`GraphView`] only — they
//! never touch [`BuildGraph`] directly.

use std::collections::{BTreeMap, BTreeSet};

use camino::Utf8PathBuf;

use crate::ir::{BuildEdge, BuildGraph};

pub mod render;
pub mod render_dot;
pub mod render_html;

/// Deterministic projection of [`BuildGraph`] consumed by renderer adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphView {
    /// Targets built when no explicit target is requested, sorted lexically.
    pub default_targets: Vec<Utf8PathBuf>,
    /// Every node referenced by the graph, sorted by [`NodeView::path`].
    pub nodes: Vec<NodeView>,
    /// Every edge, sorted by `(from, to, class)`.
    pub edges: Vec<EdgeView>,
    /// Reserved for the visualisation-bounding work tracked under roadmap
    /// item 3.15.6. Currently always `None`.
    pub limit: Option<usize>,
}

/// A node in the rendered graph. A node is either a build target produced by
/// some action, or a leaf source path referenced as an input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeView {
    /// Path identifying the node. Output paths for targets, input paths for
    /// sources.
    pub path: Utf8PathBuf,
    /// Whether the node is a target produced by an action or a leaf source.
    pub kind: NodeKind,
    /// Identifier of the producing action, when [`NodeKind::Target`].
    pub action_id: Option<String>,
    /// Optional human-readable description carried by the producing action.
    pub description: Option<String>,
}

/// Classification of a [`NodeView`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// A leaf input file referenced by one or more build edges. The node is
    /// not produced by any action in the current manifest.
    Source,
    /// A target produced by an action. `phony` and `always` mirror the
    /// corresponding flags on [`BuildEdge`].
    Target {
        /// The output is `phony` and has no on-disk artefact.
        phony: bool,
        /// The producing action runs on every invocation.
        always: bool,
    },
}

/// A directed edge in the rendered graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeView {
    /// Source of the edge.
    pub from: Utf8PathBuf,
    /// Destination of the edge.
    pub to: Utf8PathBuf,
    /// Classification of the edge.
    pub class: EdgeClass,
}

/// Dependency class of an [`EdgeView`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeClass {
    /// Explicit input → explicit output dependency.
    Explicit,
    /// Explicit input → implicit output dependency. The output is generated
    /// by the action but is not listed in its `explicit_outputs`.
    ImplicitOutput,
    /// Order-only dependency (Ninja `||`). Does not trigger a rebuild.
    OrderOnly,
}

impl GraphView {
    /// Project a [`BuildGraph`] into a deterministic [`GraphView`].
    ///
    /// The projection sorts every collection so that two graphs equal up to
    /// `HashMap` insertion order yield byte-identical views.
    #[must_use]
    pub fn from_build_graph(graph: &BuildGraph) -> Self {
        let edges_seen = collect_unique_edges(graph);

        let mut node_paths: BTreeMap<Utf8PathBuf, NodeKind> = BTreeMap::new();
        let mut node_metadata: BTreeMap<Utf8PathBuf, NodeMetadata> = BTreeMap::new();
        let mut edges: BTreeSet<EdgeView> = BTreeSet::new();

        for edge in &edges_seen {
            register_outputs(graph, edge, &mut node_paths, &mut node_metadata);
            register_inputs_and_edges(edge, &mut node_paths, &mut edges);
        }

        let nodes = node_paths
            .into_iter()
            .map(|(path, kind)| {
                let meta = node_metadata.remove(&path).unwrap_or_default();
                NodeView {
                    path,
                    kind,
                    action_id: meta.action_id,
                    description: meta.description,
                }
            })
            .collect();

        let mut default_targets = graph.default_targets.clone();
        default_targets.sort();
        default_targets.dedup();

        Self {
            default_targets,
            nodes,
            edges: edges.into_iter().collect(),
            limit: None,
        }
    }
}

#[derive(Debug, Default, Clone)]
struct NodeMetadata {
    action_id: Option<String>,
    description: Option<String>,
}

/// Deduplicate the build edges referenced by [`BuildGraph::targets`].
///
/// `BuildGraph::targets` maps every output path back to a (cloned) `BuildEdge`,
/// so iterating values produces duplicates. We dedup by the lexically-sorted
/// tuple of explicit outputs, which uniquely identifies a build statement.
fn collect_unique_edges(graph: &BuildGraph) -> Vec<BuildEdge> {
    let mut by_key: BTreeMap<Vec<Utf8PathBuf>, BuildEdge> = BTreeMap::new();
    for edge in graph.targets.values() {
        let mut key = edge.explicit_outputs.clone();
        key.sort();
        by_key.entry(key).or_insert_with(|| edge.clone());
    }
    by_key.into_values().collect()
}

fn register_outputs(
    graph: &BuildGraph,
    edge: &BuildEdge,
    node_paths: &mut BTreeMap<Utf8PathBuf, NodeKind>,
    node_metadata: &mut BTreeMap<Utf8PathBuf, NodeMetadata>,
) {
    let description = graph
        .actions
        .get(&edge.action_id)
        .and_then(|action| action.description.clone());
    let outputs = edge
        .explicit_outputs
        .iter()
        .chain(edge.implicit_outputs.iter());
    for out in outputs {
        node_paths.insert(
            out.clone(),
            NodeKind::Target {
                phony: edge.phony,
                always: edge.always,
            },
        );
        node_metadata.insert(
            out.clone(),
            NodeMetadata {
                action_id: Some(edge.action_id.clone()),
                description: description.clone(),
            },
        );
    }
}

fn register_inputs_and_edges(
    edge: &BuildEdge,
    node_paths: &mut BTreeMap<Utf8PathBuf, NodeKind>,
    edges: &mut BTreeSet<EdgeView>,
) {
    let implicit: BTreeSet<&Utf8PathBuf> = edge.implicit_outputs.iter().collect();
    for input in &edge.inputs {
        node_paths.entry(input.clone()).or_insert(NodeKind::Source);
        for out in edge
            .explicit_outputs
            .iter()
            .chain(edge.implicit_outputs.iter())
        {
            let class = if implicit.contains(out) {
                EdgeClass::ImplicitOutput
            } else {
                EdgeClass::Explicit
            };
            edges.insert(EdgeView {
                from: input.clone(),
                to: out.clone(),
                class,
            });
        }
    }
    for dep in &edge.order_only_deps {
        node_paths.entry(dep.clone()).or_insert(NodeKind::Source);
        for out in edge
            .explicit_outputs
            .iter()
            .chain(edge.implicit_outputs.iter())
        {
            edges.insert(EdgeView {
                from: dep.clone(),
                to: out.clone(),
                class: EdgeClass::OrderOnly,
            });
        }
    }
}

impl Ord for EdgeView {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&self.from, &self.to, self.class).cmp(&(&other.from, &other.to, other.class))
    }
}

impl PartialOrd for EdgeView {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests;
