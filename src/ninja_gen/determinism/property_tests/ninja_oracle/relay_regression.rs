//! Directed regression for the upstream Ninja dyndep scheduler crash.
//!
//! [`super`] states `OBL-NINJA` as a property over a generated domain. That
//! property is the obligation, but it is not the *evidence* for the emitter
//! change made to avoid [ninja-build/ninja#2662]: the drawn domain reaches the
//! crashing shape only by sampling, and a sampled green run of a rare shape
//! proves very little. This module pins the shape directly, so the repair is
//! guarded by a case that is known — measured, not hoped — to reach it.
//!
//! # The shape
//!
//! Ninja loads a dyndep file part-way through planning, and revealing a
//! dependency appends an edge to that dependency's out-edge vector. When the
//! revealed node is already scheduled and a `NodeFinished` iteration is in
//! progress over that same vector, the append can reallocate it. The condition
//! the crash actually needs, isolated one variable at a time, is an
//! **order-only** edge naming the revealed node that was loaded earlier in the
//! file; an ordinary edge to the same node does not trigger it. On Ninja
//! 1.11.1 this graph segfaults when the sidecar reveals the real dependency and
//! is accepted when it reveals the relay. Both halves were measured on the
//! *emitted* bundle, not on a hand-built model: rewriting the emitter's own
//! output into the pre-relay shape dies of signal 11.
//!
//! The target list decides whether the crash is reachable, which is why
//! [`super::workspace::BundleWorkspace`] names every explicit output rather
//! than a single one. Asking the same pre-relay bundle about `out/0008` alone
//! exits 0; asking about the full explicit-output set dies. A narrower target
//! selection here would therefore leave this regression green while proving
//! nothing, so the breadth is part of the measurement rather than tidiness.
//!
//! # What this test can and cannot separate
//!
//! The relay line itself adds a consumer of the real dependency, and a
//! *sufficient* number of consumers also hides the defect by changing the
//! vector's capacity. So a green run here does not, on its own, prove that the
//! sidecar reveals the right node — the residual capacity luck is not
//! observable through `ninja`'s exit status. The structural half of the claim
//! is asserted directly in `dyndep::tests`, which requires every sidecar to
//! name a relay and no sidecar to name a real dependency. This test carries the
//! end-to-end half: the bundle the emitter produces for this shape is one real
//! Ninja accepts, on the binary in `PATH`.
//!
//! The sample is a fixed graph rather than a drawn one, and the rebuild is not
//! asserted: `-n` and `-t commands` are what reach the loader, and they cost no
//! commands.

use super::workspace::BundleWorkspace;
use super::{emitted, ninja_is_available, paths_of, targets_of};
use crate::ast::{CommandIsolation, Recipe, StringOrList};
use crate::ir::{Action, BuildEdge, BuildGraph, DependencyOrder};
use camino::Utf8PathBuf;
use proptest::test_runner::TestCaseError;

proptest::proptest! {
    /// Real Ninja accepts the emitted bundle for the `#2662` shape.
    ///
    /// Written as a one-case property rather than a plain `#[test]` so the
    /// helper's [`TestCaseError`] result can be returned with `?`; the graph is
    /// constant, so there is nothing to sample.
    #[test]
    fn real_ninja_accepts_the_bundle_that_reaches_the_upstream_crash(_ in 0..1u8) {
        if !ninja_is_available() {
            return Ok(());
        }
        let graph = crash_shaped_graph();
        let bundle = emitted(&graph)?;
        let workspace = BundleWorkspace::write(&bundle, &paths_of(&graph), &targets_of(&graph))?;
        workspace.require_accepts()?;
    }
}

/// Build the graph whose emission reaches the `#2662` scheduler shape.
///
/// Four edges, in the order the emitter writes them:
///
/// 1. the producer of `out/0003`, which is the node a later sidecar reaches;
/// 2. an **order-only** consumer of `out/0003`, which is what makes the append
///    land inside a live iteration;
/// 3. a serial edge over `out/0006` and `out/0003`, which stages the two
///    sidecars — the second of which reaches `out/0003`;
/// 4. an ordinary consumer of `out/0003`, because the crash needs the revealed
///    node's out-edge vector to be exactly full when the sidecar appends to it.
///    Measured on Ninja 1.11.1 with the order-only edge present, counting
///    *extra* ordinary consumers beyond the producer, the order-only edge and
///    the relay: one segfaults, two do not, three segfault again, and five and
///    eight do not. The relation is not a threshold, which is why this graph is
///    pinned rather than drawn.
///
/// Edge 3 is the only serial edge with more than one implicit dependency, so it
/// is the only one lowered into gates. Everything else renders directly.
fn crash_shaped_graph() -> BuildGraph {
    let mut graph = BuildGraph::default();
    graph
        .actions
        .insert("producer".into(), command_action("echo 02"));
    graph
        .actions
        .insert("aggregate".into(), dependency_only_action());
    for edge in [
        BuildEdge {
            action_id: "producer".into(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: paths(&["out/0002", "out/0003"]),
            implicit_outputs: paths(&["out/0004"]),
            order_only_deps: Vec::new(),
            phony: false,
            always: false,
        },
        BuildEdge {
            action_id: "aggregate".into(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: paths(&["out/0006"]),
            implicit_outputs: Vec::new(),
            order_only_deps: paths(&["out/0003"]),
            phony: true,
            always: false,
        },
        BuildEdge {
            action_id: "aggregate".into(),
            inputs: Vec::new(),
            implicit_deps: paths(&["out/0006", "out/0003"]),
            dependency_order: DependencyOrder::Serial,
            explicit_outputs: paths(&["out/0008"]),
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: true,
            always: false,
        },
        BuildEdge {
            action_id: "aggregate".into(),
            inputs: Vec::new(),
            implicit_deps: paths(&["out/0003"]),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: paths(&["out/0009"]),
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: true,
            always: false,
        },
    ] {
        // A duplicate alias would silently change the graph this test exists to
        // pin, so a collision is a test failure rather than something to ignore.
        assert!(
            graph.insert_edge(edge).is_ok(),
            "the crash-shaped graph's output aliases must be unique",
        );
    }
    graph
}

/// An action that runs `command`.
fn command_action(command: &str) -> Action {
    action(Recipe::Command {
        command: StringOrList::String(command.to_owned()),
        isolation: CommandIsolation::Shared,
    })
}

/// An action with no recipe, which the emitter renders as a Ninja `phony`.
fn dependency_only_action() -> Action {
    action(Recipe::Command {
        command: StringOrList::Empty,
        isolation: CommandIsolation::Shared,
    })
}

/// Wrap `recipe` in an action carrying no optional Ninja attributes.
fn action(recipe: Recipe) -> Action {
    Action {
        recipe,
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    }
}

/// Collect `names` into graph paths.
fn paths(names: &[&str]) -> Vec<Utf8PathBuf> {
    names.iter().map(Utf8PathBuf::from).collect()
}

/// Assert the emitter actually stages this graph before Ninja sees it.
///
/// The crash is only reachable through dyndep, so a future change that stopped
/// staging would turn the test above into a vacuous green: it would still pass,
/// and would no longer be about `#2662` at all. Checking the condition here
/// makes that failure loud, and it costs no subprocess.
#[test]
fn the_crash_shaped_graph_is_really_staged() -> Result<(), TestCaseError> {
    let graph = crash_shaped_graph();
    let bundle = emitted(&graph)?;
    if bundle.dyndep_files().len() != 2 {
        return Err(TestCaseError::fail(format!(
            "the crash shape needs exactly two sidecars so the second can reach \
             out/0003, got {}",
            bundle.dyndep_files().len()
        )));
    }
    if !bundle
        .build_file()
        .contains("build out/0006: phony || out/0003")
    {
        return Err(TestCaseError::fail(format!(
            "the order-only consumer is the trigger and must be emitted \
             verbatim: {}",
            bundle.build_file()
        )));
    }
    Ok(())
}
