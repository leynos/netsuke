//! `OBL-NINJA`: emitted bundles are accepted by real Ninja, and permuted
//! emissions are semantically identical.
//!
//! Every other obligation in this plan reads the emitter's output as text. That
//! is necessary and not sufficient: an emitter could preserve byte equality
//! across permutations while producing a file real Ninja rejects, or while
//! producing the same *wrong* command list. This obligation therefore hands the
//! bundle to the actual `ninja` binary and asks it two questions.
//!
//! # Two invocations, not one
//!
//! `-t commands` resolves the graph and prints the commands it would run, but —
//! measured — it does not load dyndep sidecars. A bundle whose sidecar Ninja
//! cannot find exits zero under `-t commands` and fails only under `-n`. The
//! dyndep path is the newest and most intricate part of the emitter, so an
//! oracle using only `-t commands` would be vacuous over exactly the machinery
//! it most needs to check. Both invocations run on every case.
//!
//! # Nothing is built
//!
//! `-t commands` and `-n` do not execute a command, so the oracle is testing
//! *parsing and scheduling*, not execution. It still writes every path the
//! bundle names, because `-n` rejects an input no rule produces and reports it
//! as `missing and no known rule to make it`. Writing the declared paths as
//! empty files keeps the check on the emitter's text rather than on the
//! build's own reachability.
//!
//! # Why a fresh directory per case
//!
//! The emitter's sidecars are content-addressed. A directory reused across
//! cases would let a sidecar written for one case satisfy another case's
//! reference, so a missing-sidecar defect would pass because a *different* case
//! had written the file. Each case gets its own [`tempfile::TempDir`].
//!
//! # The domain
//!
//! One to eight actions over one to twelve edges, as the obligation states.
//! The shared [`GraphSpec`] generator supplies it through [`GraphBounds`], so
//! the well-formedness argument is stated once rather than copied. Only the
//! sampling bounds and the pool-slot ceiling differ, and the ceiling is
//! load-bearing: a drawn slot is clamped to the window of slots allocated
//! before its edge, so the shared ceiling of 200 would collapse almost every
//! dependency draw in an eight-edge domain onto the window's last slot, and the
//! dyndep path — which needs two distinct implicit dependencies on one serial
//! edge — would never be reached.
//!
//! [`GraphSpec`]: crate::ninja_gen::determinism::strategy::graph::GraphSpec
//! [`GraphBounds`]: crate::ninja_gen::determinism::strategy::graph::GraphBounds

mod workspace;

use crate::ir::{BuildEdge, BuildGraph};
use crate::ninja_gen::determinism::strategy::Classification;
use crate::ninja_gen::determinism::strategy::classification::edge_lists_differ;
use crate::ninja_gen::determinism::strategy::graph::{
    GraphBounds, GraphSpec, arb_graph_spec_within,
};
use crate::ninja_gen::generate_bundle;
use camino::Utf8PathBuf;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use std::cell::Cell;
use std::sync::OnceLock;
use test_support::ninja::{NinjaWorkspaceError, ninja_integration_workspace};
use workspace::BundleWorkspace;

/// The obligation's stated domain, with a pool ceiling it can actually reach.
const NINJA_BOUNDS: GraphBounds = GraphBounds {
    min_actions: 1,
    max_actions: 8,
    min_edges: 1,
    max_edges: 12,
    // Sixteen slots spans the largest case's pool while keeping most draws
    // inside a window; see the module documentation for why the shared 200 is
    // unsuitable.
    max_position: 16,
    min_phony_actions: 1,
};

/// Draw the obligation's domain from the shared generator.
fn arb_ninja_spec() -> impl Strategy<Value = GraphSpec> {
    arb_graph_spec_within(NINJA_BOUNDS)
}

/// Whether a usable `ninja` binary is available for this test binary.
///
/// Probed once and cached: the check spawns a subprocess, and asking it per
/// case would spend the case budget on process creation. The result is a plain
/// `bool` passed between threads by `OnceLock`, never an environment mutation.
fn ninja_is_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        match ninja_integration_workspace() {
            Ok(workspace) => {
                // The probe's directory is not reused: every case makes its
                // own, so that no case can see another's sidecars.
                drop(workspace);
                true
            }
            Err(
                NinjaWorkspaceError::ProbeSpawn(_)
                | NinjaWorkspaceError::ProbeFailed(_)
                | NinjaWorkspaceError::ProbeTimeout(_),
            ) => {
                // `ninja_integration_workspace` has already panicked when the
                // run requires the binary, so reaching here means the skip is
                // legitimate and `NETSUKE_REQUIRE_NINJA` is not set.
                tracing::warn!(
                    "skipping OBL-NINJA: no usable `ninja` binary; set \
                     NETSUKE_REQUIRE_NINJA=1 to make this a failure"
                );
                false
            }
            Err(error) => panic!("prepare the Ninja oracle workspace: {error}"),
        }
    })
}

proptest! {
    // A case costs two `ninja` invocations per insertion order, measured at
    // about 1.6 ms and 1.9 ms each, so sixty-four cases is a few seconds.
    #![proptest_config(ProptestConfig { cases: 64, .. ProptestConfig::default() })]

    /// Real Ninja accepts both insertion orders of the emitted bundle.
    ///
    /// Both orders are checked rather than only the first because the
    /// obligation is stated over a *permuted emission*: a bundle that happened
    /// to parse under one order while the other emitted something Ninja
    /// rejects would fail the claim, and checking one order would not see it.
    #[test]
    fn real_ninja_accepts_both_insertion_orders(spec in arb_ninja_spec()) {
        if !ninja_is_available() {
            return Ok(());
        }
        for order in [spec.orders().0, spec.orders().1] {
            let graph = materialize(&spec, order)?;
            let bundle = emitted(&graph)?;
            let workspace =
                BundleWorkspace::write(&bundle, &paths_of(&graph), &targets_of(&graph))?;
            workspace.require_accepts()?;
        }
    }

    /// The two insertion orders yield the same command list under real Ninja.
    ///
    /// This is the determinism claim stated through an independent oracle
    /// rather than through byte comparison. The targets are named explicitly
    /// and sorted, so the comparison is between two runs over the same target
    /// set and cannot differ merely because one invocation was asked for a
    /// different default set.
    #[test]
    fn real_ninja_yields_the_same_commands_under_both_orders(spec in arb_ninja_spec()) {
        if !ninja_is_available() {
            return Ok(());
        }
        let mut lists = Vec::new();
        for order in [spec.orders().0, spec.orders().1] {
            let graph = materialize(&spec, order)?;
            let bundle = emitted(&graph)?;
            let workspace =
                BundleWorkspace::write(&bundle, &paths_of(&graph), &targets_of(&graph))?;
            lists.push(workspace.command_list()?);
        }
        let (first, second) = (lists.first(), lists.last());
        prop_assert_eq!(
            first,
            second,
            "the command list real Ninja derives must not depend on the order \
             the graph's edges were inserted in",
        );
    }
}

/// Materialize one specification under `order`.
///
/// # Errors
///
/// Returns a test failure when the specification's pool slices collide, which
/// is unreachable in a correct generator and therefore reported rather than
/// swallowed.
fn materialize(spec: &GraphSpec, order: &[usize]) -> Result<BuildGraph, TestCaseError> {
    spec.materialize(order)
        .map_err(|error| TestCaseError::fail(format!("materialize the graph: {error}")))
}

/// Emit one materialized graph through the bundle entry point.
///
/// # Errors
///
/// Returns a test failure when the emitter refuses the graph. The generated
/// domain is well formed by construction, so refusal is a generator or emitter
/// defect rather than an expected outcome.
fn emitted(graph: &BuildGraph) -> Result<crate::ninja_gen::GeneratedNinja, TestCaseError> {
    generate_bundle(graph).map_err(|error| {
        TestCaseError::fail(format!("a well-formed generated graph must emit: {error}"))
    })
}

/// Every path a materialized graph names on disk.
///
/// Explicit and implicit outputs, inputs, implicit dependencies, and order-only
/// dependencies. Inputs are included because `-n` rejects an input no rule
/// produces, and order-only dependencies because Ninja resolves them for the
/// graph's schedule.
fn paths_of(graph: &BuildGraph) -> Vec<Utf8PathBuf> {
    let mut paths = Vec::new();
    for edge in graph.edges() {
        paths.extend(edge.explicit_outputs.iter().cloned());
        paths.extend(edge.implicit_outputs.iter().cloned());
        paths.extend(edge.inputs.iter().cloned());
        paths.extend(edge.implicit_deps.iter().cloned());
        paths.extend(edge.order_only_deps.iter().cloned());
    }
    paths
}

/// The targets the command-list comparison names, deduplicated and sorted.
///
/// Sorted because the two runs must be asked for the same set: passing a
/// different target order would make the invocation itself the difference, and
/// deduplicated because a default may legitimately repeat an output.
fn targets_of(graph: &BuildGraph) -> Vec<Utf8PathBuf> {
    let mut targets: Vec<Utf8PathBuf> = graph
        .edges()
        .flat_map(|edge| edge.explicit_outputs.iter().cloned())
        .chain(graph.default_targets.iter().cloned())
        .collect();
    targets.sort_unstable();
    targets.dedup();
    targets
}

/// Counters proving the run reached the classes the obligation names.
///
/// A tally rather than a per-case assertion: no single case need carry every
/// class, and requiring it would spend the budget on the intersection instead
/// of on coverage of each class. The obligation asks for a **non-zero case
/// count** for each, which is what this aggregates.
#[derive(Clone, Copy, Debug, Default)]
struct Reached {
    /// Cases carrying a multi-output edge.
    multi_output: usize,
    /// Cases carrying a dependency-only (Ninja `phony`) edge.
    phony: usize,
    /// Cases carrying a staged dyndep edge — a serial edge with two or more
    /// implicit dependencies.
    dyndep_staged: usize,
    /// Cases carrying a non-empty `default` line.
    defaults: usize,
    /// Cases whose two insertion orders genuinely differed.
    divergent_orders: usize,
    /// Total cases run.
    cases: usize,
}

impl Reached {
    /// Fold one case's shape into the run's tally.
    fn extended(self, classes: Classification, is_divergent: bool) -> Self {
        Self {
            multi_output: self.multi_output + usize::from(classes.multi_output_edges > 0),
            phony: self.phony + usize::from(classes.phony_edges > 0),
            dyndep_staged: self.dyndep_staged + usize::from(classes.dyndep_staged_edges > 0),
            defaults: self.defaults + usize::from(classes.default_targets > 0),
            divergent_orders: self.divergent_orders + usize::from(is_divergent),
            cases: self.cases + 1,
        }
    }
}

/// Whether two insertion orders yield genuinely different edge sequences.
///
/// The permutation is a rotation, so it differs from the first order for any
/// edge count above one; the comparison is still made rather than assumed,
/// because a case whose orders coincided would compare a thing to itself and
/// establish nothing.
fn orders_diverge(spec: &GraphSpec) -> bool {
    let (first, second) = spec.orders();
    let sequences = |order: &[usize]| -> Vec<BuildEdge> {
        spec.materialize(order)
            .map(|graph| graph.edges().cloned().collect())
            .unwrap_or_default()
    };
    edge_lists_differ(&sequences(first), &sequences(second))
}

/// The generator reaches every class `OBL-NINJA` requires.
///
/// Runs only when Ninja is present, because the obligation's own acceptance is
/// stated that way: without a binary there is no oracle, so the classes the
/// oracle would have exercised cannot be claimed. The classes are counted from
/// the specification rather than from an emitted bundle, so this check is about
/// the generator and costs no subprocess.
#[test]
fn the_domain_reaches_every_class_the_obligation_names() {
    if !ninja_is_available() {
        return;
    }
    let tallies: Cell<Reached> = Cell::new(Reached::default());
    super::support::coverage_runner(64)
        .run(&arb_ninja_spec(), |spec| {
            let diverse = orders_diverge(&spec);
            tallies.set(tallies.get().extended(spec.classification(), diverse));
            Ok(())
        })
        .expect("the OBL-NINJA graph strategy should be generatable");

    let observed = tallies.into_inner();
    macro_rules! covered {
        ($field:ident, $why:expr) => {
            assert!(observed.$field > 0, "{}, reached: {observed:?}", $why,);
        };
    }
    covered!(
        multi_output,
        "no case carried a multi-output edge, so the oracle never saw the \
         emitter's implicit-output syntax"
    );
    covered!(
        phony,
        "no case carried a dependency-only edge, so the oracle never saw a \
         rule-less `phony` build statement"
    );
    covered!(
        dyndep_staged,
        "no case carried a serial edge with two or more implicit dependencies, \
         so no sidecar was ever written and the dyndep path was never reached"
    );
    covered!(
        defaults,
        "no case declared a default target, so the `default` line was never \
         exercised"
    );
    covered!(
        divergent_orders,
        "every case's two insertion orders produced the same edge sequence, so \
         the permutation arm compared one arrangement to itself"
    );
    assert_eq!(
        observed.cases, 64,
        "every sampled case should have been tallied; reached: {observed:?}",
    );
}
