//! Compare alias traversal with an independent edge-level cycle oracle.
//!
//! The generator builds a directed acyclic backbone whose prerequisites point
//! only at lower-indexed edges or at paths no edge produces, then optionally
//! adds one back arc so the graph is cyclic. Every prerequisite kind cycle
//! detection traverses takes part, each edge splits one to three aliases
//! between explicit and implicit outputs, and the oracle in
//! [`super::edge_oracle`] derives its verdict from the edge graph alone.
//! Agreement is therefore evidence about alias resolution rather than a
//! restatement of the detector.

use camino::Utf8PathBuf;
use proptest::prelude::*;
use proptest::strategy::BoxedStrategy;

use super::super::super::graph::{BuildEdge, BuildGraph};
use super::super::analyse;
use super::edge_oracle::edge_graph_has_cycle;

/// The most output aliases one generated edge may declare.
const MAX_ALIASES: usize = 3;

/// The smallest generated graph, in edges.
const MIN_EDGES: usize = 1;

/// The largest generated graph, in edges.
const MAX_EDGES: usize = 12;

/// The most extra prerequisites one generated edge may carry.
const MAX_EXTRA_DEPS: usize = 3;

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
            // Clamp instead of rejecting so `from < to <= span` always holds
            // and every sampled graph stays usable.
            let to = from + 1 + to_seed.min(span - from - 1);
            (kind, from, to, alias)
        })
        .boxed()
}

/// Generate DAG-based graphs with every dependency kind and a back-arc variant.
///
/// Each edge takes one backbone prerequisite on the previous edge, so a back
/// arc from a lower-indexed edge to a higher-indexed one always closes a
/// cycle. Random prerequisites add branching, merging, repeated producer
/// aliases, and paths no edge produces, giving the oracle a wider domain than
/// a bare chain.
fn participation_graph_strategy() -> impl Strategy<Value = BuildGraph> {
    (MIN_EDGES..=MAX_EDGES)
        .prop_flat_map(|edge_count| {
            let inner_edges = edge_count.saturating_sub(1);
            (
                proptest::collection::vec(alias_counts_strategy(), edge_count..=edge_count),
                proptest::collection::vec(dependency_kind_strategy(), inner_edges..=inner_edges),
                proptest::collection::vec(
                    proptest::collection::vec(
                        dependency_spec_strategy(edge_count),
                        0..=MAX_EXTRA_DEPS,
                    ),
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
    if !matches!(kind, BackArcKind::ImplicitOutput) {
        return;
    }
    if let Some(counts) = alias_counts.get_mut(to) {
        *counts = counts.with_implicit();
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

    let spine = alias_counts
        .iter()
        .copied()
        .zip(backbone_kinds.iter().copied())
        .zip(dependencies.iter())
        .map(|((counts, kind), specs)| (counts, kind, specs));
    for (offset, (edge, spine_entry)) in edges.iter_mut().skip(1).zip(spine).enumerate() {
        let edge_index = offset + 1;
        let (previous_counts, kind, specs) = spine_entry;
        push_dependency(
            edge,
            alias_of(edge_index - 1, previous_counts, 0, false),
            kind,
        );
        for spec in specs {
            push_dependency(
                edge,
                dependency_path(*spec, edge_index, alias_counts),
                spec.kind,
            );
        }
    }

    if let Some((from, dependency, kind)) = back_arc_dependency(back_arc, alias_counts)
        && let Some(edge) = edges.get_mut(from)
    {
        push_dependency(edge, dependency, kind);
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
    back_arc: BackArc,
    alias_counts: &[AliasCounts],
) -> Option<(usize, Utf8PathBuf, DependencyKind)> {
    let (kind, from, to, alias) = back_arc;
    let counts = alias_counts.get(to).copied()?;
    let (dependency, dependency_kind) = match kind {
        BackArcKind::None => return None,
        BackArcKind::Input => (alias_of(to, counts, alias, false), DependencyKind::Input),
        BackArcKind::Implicit => (alias_of(to, counts, alias, false), DependencyKind::Implicit),
        BackArcKind::OrderOnly => (
            alias_of(to, counts, alias, false),
            DependencyKind::OrderOnly,
        ),
        BackArcKind::ImplicitOutput => (alias_of(to, counts, alias, true), DependencyKind::Input),
    };
    Some((from, dependency, dependency_kind))
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
    // Clamping keeps the target on a lower-indexed edge without rejecting
    // samples, which preserves the acyclic base the back arc needs.
    let target = spec.target.min(edge_index - 1);
    alias_counts.get(target).copied().map_or_else(
        || absent_path(spec.alias),
        |counts| alias_of(target, counts, spec.alias, false),
    )
}

/// Return the `ordinal`th alias of an edge, optionally insisting on an implicit one.
fn alias_of(
    edge_index: usize,
    counts: AliasCounts,
    ordinal: usize,
    is_implicit: bool,
) -> Utf8PathBuf {
    if is_implicit {
        let alias = ordinal.min(counts.implicit.saturating_sub(1));
        return implicit_output(edge_index, alias);
    }
    let position = ordinal.min(counts.total().saturating_sub(1));
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

proptest! {
    #[test]
    fn alias_traversal_agrees_with_edge_oracle(graph in participation_graph_strategy()) {
        prop_assert_eq!(analyse(&graph).cycle.is_some(), edge_graph_has_cycle(&graph));
    }
}
