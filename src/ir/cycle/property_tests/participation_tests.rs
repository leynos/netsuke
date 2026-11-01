//! Compare alias traversal with an independent edge-level cycle oracle.

use std::collections::{HashMap, HashSet, VecDeque};

use camino::Utf8PathBuf;
use proptest::prelude::*;
use proptest::strategy::BoxedStrategy;

use super::super::super::graph::{BuildEdge, BuildGraph, EdgeId};
use super::super::analyse;

/// The most output aliases one generated edge may declare.
const MAX_ALIASES: usize = 3;

#[derive(Clone, Copy, Debug)]
enum DependencyKind {
    Input,
    Implicit,
    OrderOnly,
}

#[derive(Clone, Copy, Debug)]
enum BackArcKind {
    None,
    Input,
    Implicit,
    OrderOnly,
    ImplicitOutput,
}

/// Split one edge's aliases between explicit and implicit outputs.
#[derive(Clone, Copy, Debug)]
struct AliasCounts {
    explicit: usize,
    implicit: usize,
}

impl AliasCounts {
    /// Return how many aliases the edge declares in total.
    fn total(self) -> usize {
        self.explicit + self.implicit
    }

    /// Force between one and two implicit aliases, keeping an explicit one.
    fn with_implicit(self) -> Self {
        let implicit = self.implicit.clamp(1, MAX_ALIASES - 1);
        Self {
            explicit: self.explicit.min(MAX_ALIASES - implicit),
            implicit,
        }
    }
}

/// Describe one prerequisite, which may point at a path no edge produces.
#[derive(Clone, Copy, Debug)]
struct DependencySpec {
    kind: DependencyKind,
    target: usize,
    alias: usize,
    is_absent: bool,
}

/// Describe the optional back arc that closes a generated cycle.
///
/// The fields are the arc kind, the edge that carries the dependency, the
/// higher-indexed edge it points at, and an alias ordinal within that edge.
type BackArc = (BackArcKind, usize, usize, usize);

/// Generate a dependency field for the acyclic base graph.
fn dependency_kind_strategy() -> impl Strategy<Value = DependencyKind> {
    prop_oneof![
        Just(DependencyKind::Input),
        Just(DependencyKind::Implicit),
        Just(DependencyKind::OrderOnly),
    ]
}

/// Generate an acyclic graph or a cycle closed by each supported arc class.
fn back_arc_kind_strategy() -> impl Strategy<Value = BackArcKind> {
    prop_oneof![
        Just(BackArcKind::None),
        Just(BackArcKind::Input),
        Just(BackArcKind::Implicit),
        Just(BackArcKind::OrderOnly),
        Just(BackArcKind::ImplicitOutput),
    ]
}

/// Generate one to three aliases per edge, split across both output kinds.
fn alias_counts_strategy() -> impl Strategy<Value = AliasCounts> {
    (1usize..=MAX_ALIASES, 0usize..MAX_ALIASES).prop_filter_map(
        "an edge declares between one and three aliases",
        |(explicit, implicit)| {
            (explicit + implicit <= MAX_ALIASES).then_some(AliasCounts { explicit, implicit })
        },
    )
}

/// Generate one prerequisite aimed at a lower-indexed edge or an absent path.
fn dependency_spec_strategy(edge_count: usize) -> impl Strategy<Value = DependencySpec> {
    (
        dependency_kind_strategy(),
        0usize..edge_count,
        0usize..MAX_ALIASES,
        any::<bool>(),
    )
        .prop_map(|(kind, target, alias, is_absent)| DependencySpec {
            kind,
            target,
            alias,
            is_absent,
        })
}

/// Generate a back arc whose target is always a higher-indexed edge.
fn back_arc_strategy(edge_count: usize) -> BoxedStrategy<BackArc> {
    if edge_count < 2 {
        return Just((BackArcKind::None, 0, 0, 0)).boxed();
    }
    let span = edge_count - 1;
    (
        back_arc_kind_strategy(),
        0usize..span,
        0usize..span,
        0usize..MAX_ALIASES,
    )
        .prop_map(move |(kind, from, to_seed, alias)| {
            // Resolve the seed so that `from < to < edge_count` without
            // rejection, keeping every generated graph usable.
            let from = from % span;
            let to = from + 1 + to_seed % (span - from);
            (kind, from, to, alias)
        })
        .boxed()
}

/// Generate DAG-based graphs with every dependency kind and a back-arc variant.
///
/// Each edge takes one backbone prerequisite on the previous edge, so a back
/// arc from a lower-indexed edge to a higher-indexed one always closes a cycle.
/// Random prerequisites add branching, merging, repeated producers, and paths
/// no edge produces, giving the oracle a wider domain than a bare chain.
fn participation_graph_strategy() -> impl Strategy<Value = BuildGraph> {
    (1usize..=12)
        .prop_flat_map(|edge_count| {
            let inner_edges = edge_count.saturating_sub(1);
            (
                proptest::collection::vec(alias_counts_strategy(), edge_count..=edge_count),
                proptest::collection::vec(dependency_kind_strategy(), inner_edges..=inner_edges),
                proptest::collection::vec(
                    proptest::collection::vec(dependency_spec_strategy(edge_count), 0..=3),
                    inner_edges..=inner_edges,
                ),
                back_arc_strategy(edge_count),
            )
        })
        .prop_map(
            |(mut alias_counts, backbone_kinds, dependencies, back_arc)| {
                apply_implicit_alias_requirement(&mut alias_counts, back_arc);
                build_participation_graph(&alias_counts, &backbone_kinds, &dependencies, back_arc)
            },
        )
}

/// Ensure the back arc can reach an implicit alias when that kind was chosen.
fn apply_implicit_alias_requirement(alias_counts: &mut [AliasCounts], back_arc: BackArc) {
    let (kind, _, to, _) = back_arc;
    if matches!(kind, BackArcKind::ImplicitOutput) {
        if let Some(counts) = alias_counts.get_mut(to) {
            *counts = counts.with_implicit();
        }
    }
}

/// Build an acyclic backbone with extra prerequisites and one optional back arc.
fn build_participation_graph(
    alias_counts: &[AliasCounts],
    backbone_kinds: &[DependencyKind],
    dependencies: &[Vec<DependencySpec>],
    back_arc: BackArc,
) -> BuildGraph {
    let mut edges = alias_counts
        .iter()
        .copied()
        .enumerate()
        .map(|(index, counts)| build_edge(index, counts))
        .collect::<Vec<_>>();

    for (offset, ((edge, kind), specs)) in edges
        .iter_mut()
        .skip(1)
        .zip(backbone_kinds.iter().copied())
        .zip(dependencies.iter())
        .enumerate()
    {
        let edge_index = offset + 1;
        let previous = edge_index - 1;
        push_dependency(edge, alias_of(previous, 0, alias_counts, false), kind);
        for spec in specs {
            push_dependency(
                edge,
                dependency_path(*spec, edge_index, alias_counts),
                spec.kind,
            );
        }
    }

    if let Some((from, dependency, kind)) = back_arc_dependency(back_arc, alias_counts) {
        push_dependency(&mut edges[from], dependency, kind);
    }

    let mut graph = BuildGraph::default();
    for edge in edges {
        graph
            .insert_edge(edge)
            .expect("generated output aliases must be unique");
    }
    graph
}

/// Resolve a back arc into the dependency it contributes, if any.
fn back_arc_dependency(
    (kind, from, to, alias): BackArc,
    alias_counts: &[AliasCounts],
) -> Option<(usize, Utf8PathBuf, DependencyKind)> {
    let resolved = match kind {
        BackArcKind::None => return None,
        BackArcKind::Input => (
            alias_of(to, alias, alias_counts, false),
            DependencyKind::Input,
        ),
        BackArcKind::Implicit => (
            alias_of(to, alias, alias_counts, false),
            DependencyKind::Implicit,
        ),
        BackArcKind::OrderOnly => (
            alias_of(to, alias, alias_counts, false),
            DependencyKind::OrderOnly,
        ),
        BackArcKind::ImplicitOutput => (
            alias_of(to, alias, alias_counts, true),
            DependencyKind::Input,
        ),
    };
    Some((from, resolved.0, resolved.1))
}

/// Resolve one declared prerequisite into the path it names.
fn dependency_path(
    spec: DependencySpec,
    edge_index: usize,
    alias_counts: &[AliasCounts],
) -> Utf8PathBuf {
    if spec.is_absent {
        return absent_path(spec.alias);
    }
    alias_of(spec.target % edge_index, spec.alias, alias_counts, false)
}

/// Return the `ordinal`th alias of an edge, optionally insisting on an implicit one.
fn alias_of(
    edge_index: usize,
    ordinal: usize,
    alias_counts: &[AliasCounts],
    is_implicit: bool,
) -> Utf8PathBuf {
    let counts = alias_counts[edge_index];
    if is_implicit {
        return implicit_output(edge_index, ordinal % counts.implicit);
    }
    let position = ordinal % counts.total();
    if position < counts.explicit {
        explicit_output(edge_index, position)
    } else {
        implicit_output(edge_index, position - counts.explicit)
    }
}

/// Build one edge with its declared explicit and implicit outputs.
fn build_edge(index: usize, counts: AliasCounts) -> BuildEdge {
    BuildEdge {
        action_id: format!("edge-{index}"),
        progress_label: None,
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: crate::ir::DependencyOrder::Parallel,
        explicit_outputs: (0..counts.explicit)
            .map(|alias| explicit_output(index, alias))
            .collect(),
        implicit_outputs: (0..counts.implicit)
            .map(|alias| implicit_output(index, alias))
            .collect(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    }
}

/// Return one explicit output alias for an edge.
fn explicit_output(index: usize, alias: usize) -> Utf8PathBuf {
    if alias == 0 {
        Utf8PathBuf::from(format!("n{index}"))
    } else {
        Utf8PathBuf::from(format!("n{index}.alt{alias}"))
    }
}

/// Return one implicit output alias for an edge.
fn implicit_output(index: usize, alias: usize) -> Utf8PathBuf {
    Utf8PathBuf::from(format!("n{index}.extra{alias}"))
}

/// Return an absent prerequisite path that no generated edge produces.
fn absent_path(ordinal: usize) -> Utf8PathBuf {
    Utf8PathBuf::from(format!("absent{ordinal}"))
}

/// Store `dependency` in the selected dependency field.
fn push_dependency(edge: &mut BuildEdge, dependency: Utf8PathBuf, kind: DependencyKind) {
    match kind {
        DependencyKind::Input => edge.inputs.push(dependency),
        DependencyKind::Implicit => edge.implicit_deps.push(dependency),
        DependencyKind::OrderOnly => edge.order_only_deps.push(dependency),
    }
}

/// Check for a cycle in the edge graph using Kahn's topological sort.
fn edge_graph_has_cycle(graph: &BuildGraph) -> bool {
    let edge_ids = indexed_edge_ids(graph);
    let (indegrees, successors) = edge_graph_adjacency(graph, &edge_ids);
    kahn_reports_cycle(edge_ids.len(), indegrees, &successors)
}

/// Map each generated edge's primary output to its stable graph identifier.
fn indexed_edge_ids(graph: &BuildGraph) -> Vec<EdgeId> {
    graph
        .edges()
        .map(|edge| {
            let output = edge
                .explicit_outputs
                .first()
                .expect("generated edges have an explicit output");
            graph
                .edge_id_for_output(output.as_path())
                .expect("every generated output alias is indexed")
        })
        .collect()
}

/// Build successor sets and in-degrees for the independent edge-level oracle.
fn edge_graph_adjacency(
    graph: &BuildGraph,
    edge_ids: &[EdgeId],
) -> (HashMap<EdgeId, usize>, HashMap<EdgeId, HashSet<EdgeId>>) {
    let mut indegrees = edge_ids
        .iter()
        .copied()
        .map(|edge_id| (edge_id, 0usize))
        .collect::<HashMap<_, _>>();
    let mut successors = edge_ids
        .iter()
        .copied()
        .map(|edge_id| (edge_id, HashSet::new()))
        .collect::<HashMap<_, _>>();

    for (edge, source) in graph.edges().zip(edge_ids.iter().copied()) {
        let dependencies = edge
            .inputs
            .iter()
            .chain(&edge.implicit_deps)
            .chain(&edge.order_only_deps);
        for dependency in dependencies {
            let Some(target) = graph.edge_id_for_output(dependency.as_path()) else {
                continue;
            };
            let inserted = successors
                .get_mut(&source)
                .expect("every edge has a successor set")
                .insert(target);
            if inserted {
                *indegrees
                    .get_mut(&target)
                    .expect("every dependency edge has an in-degree") += 1;
            }
        }
    }

    (indegrees, successors)
}

/// Return whether Kahn's queue leaves any edge in the graph.
fn kahn_reports_cycle(
    edge_count: usize,
    mut indegrees: HashMap<EdgeId, usize>,
    successors: &HashMap<EdgeId, HashSet<EdgeId>>,
) -> bool {
    let mut ready = indegrees
        .iter()
        .filter_map(|(edge_id, degree)| (*degree == 0).then_some(*edge_id))
        .collect::<VecDeque<_>>();
    let mut removed = 0;
    while let Some(edge_id) = ready.pop_front() {
        removed += 1;
        for successor in successors
            .get(&edge_id)
            .expect("every edge has a successor set")
        {
            let degree = indegrees
                .get_mut(successor)
                .expect("every successor has an in-degree");
            *degree -= 1;
            if *degree == 0 {
                ready.push_back(*successor);
            }
        }
    }

    removed != edge_count
}

proptest! {
    #[test]
    fn alias_traversal_agrees_with_edge_oracle(graph in participation_graph_strategy()) {
        prop_assert_eq!(analyse(&graph).cycle.is_some(), edge_graph_has_cycle(&graph));
    }
}
