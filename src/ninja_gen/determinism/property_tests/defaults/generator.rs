//! The `OBL-DEFAULT` generator, and the graph fixtures it composes.
//!
//! Split from the suite because generating a graph and asserting about what
//! was emitted are two jobs: the parent states the properties and reads the
//! text, and this module answers "what does a generated default declaration
//! look like". The split is also what keeps both files inside the repository's
//! line cap.
//!
//! Everything here is a *fixture for emission* rather than a model of it.
//! [`DefaultCase::build`] inserts real edges into a real [`BuildGraph`], so the
//! suite measures the production emitter rather than a restatement of it.

use crate::ast::{CommandIsolation, Recipe, StringOrList};
use crate::ir::{Action, BuildEdge, BuildGraph, DependencyOrder};
use camino::Utf8PathBuf;
use proptest::prelude::*;

/// Greatest number of `build` statements a generated graph carries.
///
/// `OBL-DEFAULT` states its domain as 1 to 20 edges. The bound is a cost budget
/// rather than a correctness one: the positional assertion needs the `default`
/// block to have *several* `build` lines above it, and twenty is enough to make
/// a misplacement unambiguous without making 128 cases expensive.
pub(super) const MAX_DEFAULT_EDGES: usize = 20;

/// Greatest number of distinct default targets one case may name.
pub(super) const MAX_DEFAULT_SLOTS: usize = 6;

/// Greatest number of copies appended to force the duplicate-bearing class.
pub(super) const MAX_DEFAULT_REPEATS: usize = 3;

/// The target counts every run must contain, drawn with equal probability.
///
/// Uniform draws over `0..=6` reach the empty and singleton classes only a few
/// per cent of the time, which is the shape of a flaky non-vacuity assertion
/// rather than a property. Naming the counts instead guarantees each class on
/// roughly a fifth of the cases, and the longest entry keeps a 3-way sort with
/// distinct elements reachable so the ascending and descending arrangements are
/// genuinely observable.
///
/// A slice rather than an array so [`prop::sample::select`] can take it
/// directly; indexing it would trip the workspace `indexing_slicing` deny.
const DEFAULT_SHAPES: &[usize] = &[0, 1, 2, 3, 6];

/// One generated graph together with the defaults it declares.
///
/// `Debug` is required rather than incidental: Proptest renders a failing case
/// by formatting it, so a case type that cannot be printed turns every failure
/// into a compile error at the macro instead of a shrinking counter-example.
#[derive(Debug)]
pub(super) struct DefaultCase {
    /// The graph, with `default_targets` already set to `declared`.
    pub(super) graph: BuildGraph,
    /// The default targets as declared, in declaration order.
    pub(super) declared: Vec<Utf8PathBuf>,
}

impl DefaultCase {
    /// Build a graph of `edge_count` single-output edges naming `picks`.
    ///
    /// Every edge owns one pool slot, so the drawn slot numbers are exactly the
    /// indices of the outputs the graph declares and the defaults are a
    /// sub-multiset of them by construction rather than by filtering.
    pub(super) fn build(edge_count: usize, picks: &[usize]) -> Self {
        let mut graph = BuildGraph::default();
        graph.actions.insert("act".to_owned(), action());
        for index in 0..edge_count {
            graph
                .insert_edge(edge(index))
                .expect("each edge here owns its own distinct output");
        }
        let declared: Vec<Utf8PathBuf> = picks.iter().copied().map(output_path).collect();
        graph.default_targets.clone_from(&declared);
        Self { graph, declared }
    }
}

/// Generate a graph whose default declarations cover every shape the obligation
/// names.
///
/// The declaration is built from drawn *pool slot numbers*, which is how a
/// sub-multiset of the graph's own outputs is obtained without rejection: any
/// slot number names an output the graph declares, and one slot may be drawn
/// twice. The slot count is drawn from `0..=MAX_DEFAULT_SLOTS`, empty included:
/// taking a fixed count of a vector that shrinking may shorten would leave the
/// empty class reachable only by shrink, and the class exists precisely so the
/// "no `default` line" arm runs on ordinary cases. The repeat term appends up
/// to [`MAX_DEFAULT_REPEATS`] copies of an already-drawn slot, which is what
/// makes the duplicate-bearing class guaranteed rather than merely likely.
pub(super) fn arb_default_case() -> impl Strategy<Value = DefaultCase> {
    (1usize..=MAX_DEFAULT_EDGES)
        .prop_flat_map(|edge_count| {
            (
                Just(edge_count),
                prop::sample::select(DEFAULT_SHAPES),
                prop::collection::vec(0usize..edge_count, 0..=MAX_DEFAULT_SLOTS),
                (0usize..edge_count, 0usize..=MAX_DEFAULT_REPEATS),
            )
        })
        .prop_map(
            |(edge_count, slot_count, drawn, (repeat_of, repeat_count))| {
                let picks = picks_from(drawn, slot_count, repeat_of, repeat_count);
                DefaultCase::build(edge_count, &picks)
            },
        )
}

/// The pool slots one case's defaults name, after the repeat term is applied.
///
/// Separated from the strategy so the index arithmetic sits in one place. The
/// drawn slots are truncated to `slot_count` because shrinking may shorten the
/// vector, and the repeat term appends copies of an already-drawn slot.
fn picks_from(
    drawn: Vec<usize>,
    slot_count: usize,
    repeat_of: usize,
    repeat_count: usize,
) -> Vec<usize> {
    let mut picks: Vec<usize> = drawn.into_iter().take(slot_count).collect();
    if repeat_count > 0 {
        if picks.is_empty() {
            picks.push(repeat_of);
        }
        // `is_multiple_of` then `get` rather than `repeat_of % picks.len()`
        // indexing: the workspace denies `integer_division_remainder_used` and
        // `indexing_slicing`. Picking by parity stays a function of the slot
        // drawn, so shrinking still moves one number at a time.
        let index = usize::from(!repeat_of.is_multiple_of(2));
        let repeated = picks.get(index).copied().unwrap_or(repeat_of);
        picks.resize(picks.len() + repeat_count, repeated);
    }
    picks
}

/// One executable action, shared by every edge of a generated graph.
///
/// The recipe is non-empty because the emitter rejects a programmatic action
/// with an empty command, and a `phony` action would drop its rule and remove
/// the very `build` lines the positional assertion compares against.
pub(super) fn action() -> Action {
    Action {
        recipe: Recipe::Command {
            command: StringOrList::String("echo 00".to_owned()),
            isolation: CommandIsolation::Shared,
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    }
}

/// One edge producing pool slot `index`.
pub(super) fn edge(index: usize) -> BuildEdge {
    BuildEdge {
        action_id: "act".to_owned(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: vec![output_path(index)],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    }
}

/// The path naming pool slot `index`.
///
/// The generated names are whitespace- and control-character-free, which is
/// what makes splitting the emitted `default` line on whitespace exact here
/// rather than merely convenient: no operand can need Ninja escaping.
pub(super) fn output_path(index: usize) -> Utf8PathBuf {
    Utf8PathBuf::from(format!("out/{index:04}"))
}
