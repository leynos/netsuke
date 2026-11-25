//! Generated multi-alias graph descriptions for the cycle-analysis properties.
//!
//! The generator builds an acyclic backbone of edges that each declare at least
//! two explicit aliases and one absent dependency, then optionally adds one
//! forward back arc. Keeping construction separate lets the property module
//! state the bounds it checks without also owning the generator.

use camino::Utf8PathBuf;
use proptest::prelude::*;

use super::super::super::super::graph::{BuildEdge, BuildGraph};

/// Fewest edges in a generated graph.
const MIN_EDGES: usize = 2;

/// Greatest number of edges in a generated graph.
const MAX_EDGES: usize = 6;

/// Fewest explicit aliases each edge declares.
///
/// Two or more aliases per edge is what makes the alias-amplification bound
/// non-vacuous.
const MIN_EXPLICIT_ALIASES: usize = 2;

/// Greatest number of explicit aliases each edge declares.
const MAX_EXPLICIT_ALIASES: usize = 3;

/// Greatest number of implicit aliases each edge declares.
const MAX_IMPLICIT_ALIASES: usize = 2;

/// Greatest number of extra prerequisites one edge may carry.
const MAX_EXTRA_DEPS: usize = 3;

/// Exclusive bound for a requested alias ordinal.
const MAX_ALIAS_ORDINAL: usize = MAX_EXPLICIT_ALIASES + MAX_IMPLICIT_ALIASES;

/// One dependency field an edge may declare.
#[derive(Clone, Copy, Debug)]
enum DependencyKind {
    Input,
    Implicit,
}

/// How many aliases one edge declares, split by output kind.
#[derive(Clone, Copy, Debug)]
pub(super) struct AliasCounts {
    /// Explicit output aliases declared by the edge.
    pub(super) explicit: usize,
    /// Implicit output aliases declared by the edge.
    pub(super) implicit: usize,
}

impl AliasCounts {
    /// Return how many aliases the edge declares in total.
    fn total(self) -> usize {
        self.explicit + self.implicit
    }
}

/// One generated prerequisite, possibly naming a path no edge produces.
#[derive(Clone, Copy, Debug)]
pub(super) struct DependencySpec {
    kind: DependencyKind,
    target: usize,
    alias: usize,
    is_absent: bool,
}

/// The optional arc that closes a generated cycle.
#[derive(Clone, Copy, Debug)]
pub(super) struct BackArcSpec {
    /// Dependency class carrying the arc.
    pub(super) kind: BackArcKind,
    /// Edge that carries the arc.
    pub(super) from: usize,
    /// Higher-indexed edge the arc points at.
    pub(super) to: usize,
    /// Alias ordinal requested on the target edge.
    pub(super) alias: usize,
}

/// The dependency class carrying a generated back arc.
#[derive(Clone, Copy, Debug)]
pub(super) enum BackArcKind {
    /// No back arc, leaving the graph acyclic.
    None,
    /// Explicit input on the carrying edge.
    Input,
    /// Implicit dependency on the carrying edge.
    Implicit,
    /// Explicit input naming an implicit output alias.
    ImplicitOutput,
}

/// Generate one of the supported dependency fields.
fn dependency_kind_strategy() -> impl Strategy<Value = DependencyKind> {
    prop_oneof![Just(DependencyKind::Input), Just(DependencyKind::Implicit)]
}

/// Generate two or three explicit aliases and up to two implicit aliases.
fn alias_counts_strategy() -> impl Strategy<Value = AliasCounts> {
    (
        MIN_EXPLICIT_ALIASES..=MAX_EXPLICIT_ALIASES,
        0usize..=MAX_IMPLICIT_ALIASES,
    )
        .prop_map(|(explicit, implicit)| AliasCounts { explicit, implicit })
}

/// Generate one prerequisite aimed at a lower-indexed edge or an absent path.
fn dependency_spec_strategy(edge_count: usize) -> impl Strategy<Value = DependencySpec> {
    (
        dependency_kind_strategy(),
        0usize..edge_count,
        0usize..MAX_ALIAS_ORDINAL,
        any::<bool>(),
    )
        .prop_map(|(kind, target, alias, is_absent)| DependencySpec {
            kind,
            target,
            alias,
            is_absent,
        })
}

/// Generate one of the supported back-arc classes.
fn back_arc_kind_strategy() -> impl Strategy<Value = BackArcKind> {
    prop_oneof![
        Just(BackArcKind::None),
        Just(BackArcKind::Input),
        Just(BackArcKind::Implicit),
        Just(BackArcKind::ImplicitOutput),
    ]
}

/// Generate a back arc from a lower-indexed edge to a higher-indexed one.
fn back_arc_strategy(edge_count: usize) -> impl Strategy<Value = BackArcSpec> {
    let last_index = edge_count.saturating_sub(1);
    (back_arc_kind_strategy(), 0usize..last_index)
        .prop_flat_map(move |(kind, from)| {
            let span = last_index.saturating_sub(from);
            (
                Just(kind),
                Just(from),
                0usize..span.max(1),
                0usize..MAX_ALIAS_ORDINAL,
            )
        })
        .prop_map(move |(kind, from, to_seed, alias)| BackArcSpec {
            kind,
            from,
            to: (from + 1 + to_seed).min(last_index),
            alias,
        })
}

/// Generate a whole multi-alias graph description.
pub(super) fn graph_strategy()
-> impl Strategy<Value = (Vec<AliasCounts>, Vec<Vec<DependencySpec>>, BackArcSpec)> {
    (MIN_EDGES..=MAX_EDGES).prop_flat_map(|edge_count| {
        (
            proptest::collection::vec(alias_counts_strategy(), edge_count..=edge_count),
            proptest::collection::vec(
                proptest::collection::vec(dependency_spec_strategy(edge_count), 0..=MAX_EXTRA_DEPS),
                edge_count..=edge_count,
            ),
            back_arc_strategy(edge_count),
        )
    })
}

/// Resolve the `ordinal`th alias of an edge, preferring an implicit one when
/// the back arc asks for it.
fn alias_of(
    edge_index: usize,
    counts: AliasCounts,
    ordinal: usize,
    prefer_implicit: bool,
) -> Utf8PathBuf {
    if prefer_implicit && counts.implicit > 0 {
        return implicit_output(edge_index, ordinal.min(counts.implicit - 1));
    }
    let ordinal = ordinal.min(counts.total().saturating_sub(1));
    if ordinal < counts.explicit {
        explicit_output(edge_index, ordinal)
    } else {
        implicit_output(edge_index, ordinal - counts.explicit)
    }
}

/// Return one explicit output alias for an edge.
fn explicit_output(edge_index: usize, ordinal: usize) -> Utf8PathBuf {
    Utf8PathBuf::from(format!("n{edge_index}a{ordinal}"))
}

/// Return one implicit output alias for an edge.
fn implicit_output(edge_index: usize, ordinal: usize) -> Utf8PathBuf {
    Utf8PathBuf::from(format!("n{edge_index}i{ordinal}"))
}

/// Return the absent path declared by one generated edge.
fn absent_path(edge_index: usize, ordinal: usize) -> Utf8PathBuf {
    Utf8PathBuf::from(format!("absent-{edge_index}-{ordinal}"))
}

/// Store `dependency` in the selected dependency field.
fn push_dependency(edge: &mut BuildEdge, dependency: Utf8PathBuf, kind: DependencyKind) {
    match kind {
        DependencyKind::Input => edge.inputs.push(dependency),
        DependencyKind::Implicit => edge.implicit_deps.push(dependency),
    }
}

/// Build one edge with its declared aliases and no dependencies yet.
fn build_edge(edge_index: usize, counts: AliasCounts) -> BuildEdge {
    BuildEdge {
        action_id: format!("edge-{edge_index}"),
        progress_label: None,
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: crate::ir::DependencyOrder::Parallel,
        explicit_outputs: (0..counts.explicit)
            .map(|ordinal| explicit_output(edge_index, ordinal))
            .collect(),
        implicit_outputs: (0..counts.implicit)
            .map(|ordinal| implicit_output(edge_index, ordinal))
            .collect(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
        output_visibility: crate::ir::EdgeOutputVisibility::Buffered,
    }
}

/// Build the graph described by `alias_counts`, `extra_deps`, and `back_arc`.
pub(super) fn build_graph(
    alias_counts: &[AliasCounts],
    extra_deps: &[Vec<DependencySpec>],
    back_arc: BackArcSpec,
) -> BuildGraph {
    let mut edges: Vec<BuildEdge> = alias_counts
        .iter()
        .copied()
        .enumerate()
        .map(|(edge_index, counts)| build_edge(edge_index, counts))
        .collect();

    for (edge_index, edge) in edges.iter_mut().enumerate() {
        // Every edge names one absent path, so the missing-record bound is
        // exercised even when a generated back arc is absent.
        edge.inputs.push(absent_path(edge_index, 0));
        if edge_index == 0 {
            continue;
        }
        let Some(previous) = alias_counts.get(edge_index - 1).copied() else {
            continue;
        };
        edge.inputs
            .push(alias_of(edge_index - 1, previous, 0, false));
        for spec in extra_deps.get(edge_index).into_iter().flatten() {
            let target = spec.target.min(edge_index - 1);
            let dependency = alias_counts.get(target).copied().map_or_else(
                || absent_path(edge_index, spec.alias),
                |counts| {
                    if spec.is_absent {
                        absent_path(edge_index, spec.alias)
                    } else {
                        alias_of(target, counts, spec.alias, false)
                    }
                },
            );
            push_dependency(edge, dependency, spec.kind);
        }
    }

    apply_back_arc(&mut edges, alias_counts, back_arc);

    let mut graph = BuildGraph::default();
    for edge in edges {
        graph
            .insert_edge(edge)
            .expect("generated output aliases must be unique");
    }
    graph
}

/// Append the optional back arc to its carrying edge.
fn apply_back_arc(edges: &mut [BuildEdge], alias_counts: &[AliasCounts], back_arc: BackArcSpec) {
    let (prefer_implicit, kind) = match back_arc.kind {
        BackArcKind::None => return,
        BackArcKind::Input => (false, DependencyKind::Input),
        BackArcKind::Implicit => (false, DependencyKind::Implicit),
        BackArcKind::ImplicitOutput => (true, DependencyKind::Input),
    };
    let Some(counts) = alias_counts.get(back_arc.to).copied() else {
        return;
    };
    let Some(edge) = edges.get_mut(back_arc.from) else {
        return;
    };
    let dependency = alias_of(back_arc.to, counts, back_arc.alias, prefer_implicit);
    push_dependency(edge, dependency, kind);
}

/// Sum every declared `inputs` and `implicit_deps` entry in the graph.
pub(super) fn declared_entries(graph: &BuildGraph) -> usize {
    graph
        .edges()
        .map(|edge| edge.inputs.len() + edge.implicit_deps.len())
        .sum()
}
