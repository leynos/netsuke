//! `OBL-NOLOSS`: every declared output survives from the graph to the text.
//!
//! The failure this obligation exists to catch is an edge being dropped
//! between the graph and the emitted file, taking its outputs with it. The
//! assertion is therefore about *outputs*, not about edges or statements:
//! every output the graph declares must appear exactly once on a left-hand
//! side in the emitted text.
//!
//! Counting emission lines is deliberately not the assertion. A staged serial
//! edge emits `1 + 2N` `build` statements — a sidecar and a gate per implicit
//! dependency, plus one for the edge itself — so the number of statements
//! exceeds the number of edges by `2N` on exactly the graphs this obligation
//! cares about. An equality between those two counts is false, and a property
//! built on it would be unsatisfiable rather than rigorous.
//!
//! The entry point is [`generate_bundle`], not [`crate::ninja_gen::generate`].
//! The generated domain reaches the class intersection on purpose, so
//! `graph_requires_dyndep` holds and the simple path rejects those graphs with
//! `Err(DyndepFilesRequired)`; using it would turn every interesting case into
//! a clean-looking error and every uninteresting one into the only evidence.
//!
//! Neither the assertion nor its oracle can be satisfied by the other half of
//! the emitter restoring a dropped edge's output. Gates and sidecars are
//! written under `.netsuke/serial` and `.netsuke/dyndep`, which
//! `reject_reserved_paths` keeps clear of every path the generator can name, so
//! no gate or sidecar can ever supply a user output's left-hand side. A dropped
//! edge's outputs disappear from the text and nothing puts them back.

use super::support::materialize;
use crate::ast::{CommandIsolation, Recipe, StringOrList};
use crate::ir::{Action, BuildEdge, BuildGraph, DependencyOrder, IrGenError};
use crate::ninja_gen::determinism::strategy::graph::arb_graph_spec;
use crate::ninja_gen::generate_bundle;
use camino::Utf8PathBuf;
use proptest::prelude::*;
use proptest::test_runner::{TestCaseError, TestRunner};
use std::cell::Cell;
use std::collections::BTreeMap;

proptest! {
    // Emission is string building rather than compilation, but a case can hold
    // 100 edges and 200 outputs, so the budget is below the key suite's.
    #![proptest_config(ProptestConfig { cases: 128, .. ProptestConfig::default() })]

    /// Every output the graph declares appears exactly once in the emitted text.
    ///
    /// The expected side is read from the graph — the emitter's *input* — so
    /// the comparison is against the graph rather than against a second run of
    /// the code under test. The emitted side is parsed out of the `build`
    /// lines, which is what a consumer of the file actually sees.
    #[test]
    fn every_explicit_output_appears_exactly_once(spec in arb_graph_spec()) {
        let (first, _) = spec.orders();
        let graph = materialize(&spec, first)?;
        let text = emit(&graph)?;
        let emitted = lhs_counts(&text);
        let declared = declared_outputs(&graph);
        for output in &declared {
            prop_assert_eq!(
                emitted.get(output).copied().unwrap_or(0),
                1,
                "output {} must head exactly one left-hand side; the graph \
                 declares {} outputs and the text carries {} left-hand-side \
                 entries",
                output,
                declared.len(),
                emitted.values().sum::<usize>(),
            );
        }
    }
}

/// An output-less edge is rendered rather than dropped.
///
/// The property above holds over the generated domain, which is well-formed by
/// construction. This case pins what the emitter does with the one
/// non-well-formed shape a caller can actually reach, so that "well-formed" is
/// a measured boundary rather than an assumed one.
///
/// It is reachable through the loader, not merely by construction: an empty or
/// value-absent `name:` lowers to [`StringOrList::Empty`], `to_paths` maps that
/// to `[]`, and a duplicate scan over an empty slice finds nothing to reject.
/// Real `ninja` rejects the resulting `build` line, which is why the generated
/// domain excludes this shape — but rejection by `ninja` is not the same
/// failure as a silent drop by `netsuke`, and only the latter is this
/// obligation's subject.
#[test]
fn an_output_less_edge_is_emitted_rather_than_dropped() {
    let with_implicit = graph_of_one_edge(BuildEdge {
        implicit_outputs: vec![Utf8PathBuf::from("out/implicit")],
        ..no_output_edge()
    })
    .expect("an empty explicit-output list is currently accepted at insertion");
    let text = emit(&with_implicit).expect("a graph with one output-less edge must emit");
    assert_eq!(
        lhs_counts(&text).get("out/implicit").copied().unwrap_or(0),
        1,
        "the output-less edge's implicit output is declared by the graph, so \
         the invariant applies to it unchanged. Emitted text:\n{text}",
    );

    // With no outputs of either kind there is nothing left to count, so the
    // assertion becomes that the edge reaches the text at all: a `build` line
    // with an empty left-hand side is the observable form of "not dropped",
    // and a missing line is the failure this obligation exists to catch.
    let bare = graph_of_one_edge(no_output_edge())
        .expect("a fully output-less edge is currently accepted at insertion");
    let bare_text = emit(&bare).expect("a graph with one output-less edge must emit");
    assert_eq!(
        bare_text
            .lines()
            .filter(|line| line.starts_with("build "))
            .count(),
        1,
        "the output-less edge must still reach the text; dropping it silently \
         is the failure OBL-NOLOSS exists to catch. Emitted text:\n{bare_text}",
    );
}

/// Two edges with permuted output lists are rejected, not merged.
///
/// This is the other shape the plan named, and it has the opposite answer to
/// the output-less edge. Two edges whose output lists are permutations of one
/// another have the same output *set*, so the second is refused at insertion
/// for the same reason the loader refuses it — the plan's earlier wording said
/// a direct constructor permits it, which is wrong and is corrected there.
///
/// The rejection is what makes a `path_key` collision unreachable among
/// accepted edges: sorting and joining is a lossless encoding of an output set
/// exactly because no two accepted edges share one. `OBL-PATHKEY` supplies the
/// encoding argument; this case supplies the boundary that lets it be applied.
#[test]
fn a_permuted_output_repeat_is_rejected_at_insertion() {
    let mut graph = graph_of_one_edge(permuted_edge(["out/a", "out/b"]))
        .expect("the first edge's outputs must be fresh");
    let error = graph
        .insert_edge(permuted_edge(["out/b", "out/a"]))
        .expect_err("the second edge aliases the first and must be rejected");
    assert!(
        matches!(&error, IrGenError::DuplicateOutput { outputs, .. }
            if outputs.contains(&"out/b".to_owned())),
        "a permuted repeat must be reported as a duplicate output naming the \
         aliased path, got {error:?}",
    );
}

/// The action identifier every directed case's edge refers to.
const ACTION: &str = "act";

/// Declare one action, then insert `edge` into the empty graph.
///
/// Returns the insertion result so each directed case can assert the answer
/// that is specific to it — acceptance for the output-less shape, rejection
/// for the aliasing one — rather than sharing a helper that presumes one.
fn graph_of_one_edge(edge: BuildEdge) -> Result<BuildGraph, IrGenError> {
    let mut graph = BuildGraph::default();
    graph.actions.insert(
        ACTION.to_owned(),
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
        },
    );
    graph.insert_edge(edge)?;
    Ok(graph)
}

/// An edge that declares no outputs of either kind.
fn no_output_edge() -> BuildEdge {
    BuildEdge {
        action_id: ACTION.to_owned(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: Vec::new(),
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    }
}

/// One edge producing `outputs`, in the order given.
fn permuted_edge(outputs: [&str; 2]) -> BuildEdge {
    BuildEdge {
        action_id: ACTION.to_owned(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: outputs.into_iter().map(Utf8PathBuf::from).collect(),
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    }
}

/// Declared outputs, explicit first so a failure names them in graph order.
fn declared_outputs(graph: &BuildGraph) -> Vec<String> {
    graph
        .edges()
        .flat_map(|edge| {
            edge.explicit_outputs
                .iter()
                .chain(&edge.implicit_outputs)
                .map(|path| path.as_str().to_owned())
        })
        .collect()
}

/// Count how often each path heads a `build` line.
///
/// Both halves of a left-hand side are counted: a path after the `|` marker is
/// an implicit output and is just as lost as an explicit one if its edge
/// vanishes. Splitting at the first `": "` is unambiguous because the
/// whole-graph validator rejects `:` in every path, so nothing before the
/// action name can contain one. For the same reason every generated path is
/// whitespace-free, which is what makes `split_whitespace` exact here rather
/// than merely convenient.
fn lhs_counts(text: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("build ") else {
            continue;
        };
        let Some((lhs, _)) = rest.split_once(": ") else {
            continue;
        };
        for token in lhs.split_whitespace().filter(|token| *token != "|") {
            *counts.entry(token.to_owned()).or_insert(0) += 1;
        }
    }
    counts
}

/// Emit one graph, reporting a rejection as a shrinkable test failure.
///
/// A rejection is a generator defect rather than a property failure: the pool's
/// slices are disjoint, its names are outside every reserved namespace, and its
/// characters are printable, so a well-formed graph must emit. Reporting it as
/// a failure keeps the specification's shape in the diagnostic instead of
/// letting a panic escape shrinking.
fn emit(graph: &BuildGraph) -> Result<String, TestCaseError> {
    generate_bundle(graph)
        .map(|bundle| bundle.build_file().to_owned())
        .map_err(|error| {
            TestCaseError::fail(format!("a well-formed generated graph must emit: {error}"))
        })
}

/// How many cases carried a shape this obligation needs to be meaningful.
///
/// The property is only interesting on a graph with an edge owning more than
/// one output, because that is where a lost edge is distinguishable from a lost
/// output. It is separately interesting on a graph with a staged serial edge,
/// because that is the shape whose `1 + 2N` statement count forbids the
/// count-based statement this obligation replaced. Neither shape is guaranteed
/// in any single case, so the tally is evidence about the run rather than an
/// assertion about a case — the same form as `EP-M3`'s class tally, for the
/// same reason.
///
/// The tallies live in a `Cell` because `TestRunner::run` takes an `Fn`.
#[test]
fn the_run_reaches_multi_output_and_staged_graphs() {
    let tallies: Cell<Shapes> = Cell::new(Shapes::default());
    TestRunner::new(ProptestConfig {
        cases: 128,
        ..ProptestConfig::default()
    })
    .run(&arb_graph_spec(), |spec| {
        let classes = spec.classification();
        tallies.set(
            tallies
                .get()
                .extended(classes.multi_output_edges, classes.dyndep_staged_edges),
        );
        Ok(())
    })
    .expect("the graph specification should be generatable");

    let observed = tallies.into_inner();
    assert!(
        observed.multi_output_cases > 0,
        "no case carried a multi-output edge, so a lost edge and a lost output \
         were never distinguishable: {observed:?}",
    );
    assert!(
        observed.staged_cases > 0,
        "no case carried a staged serial edge, so the 1 + 2N statement count \
         the obligation warns about was never exercised: {observed:?}",
    );
}

/// What a run reached, tallied over its cases.
#[derive(Clone, Copy, Debug, Default)]
struct Shapes {
    /// Cases carrying at least one edge with more than one output.
    multi_output_cases: usize,
    /// Cases carrying at least one dyndep-staged serial edge.
    staged_cases: usize,
}

impl Shapes {
    /// Fold one case's class counts into the tally.
    fn extended(self, multi_output_edges: usize, dyndep_staged_edges: usize) -> Self {
        Self {
            multi_output_cases: self.multi_output_cases + usize::from(multi_output_edges > 0),
            staged_cases: self.staged_cases + usize::from(dyndep_staged_edges > 0),
        }
    }
}
