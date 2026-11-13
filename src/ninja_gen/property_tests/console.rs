//! Check console scheduling across generated user and serial edges.
//!
//! Each property generates a serial parent edge plus parallel sibling edges
//! and inspects the exact `pool` binding of every emitted statement. Coverage
//! includes the inherited case where the shared action already selects the
//! console pool, so a generated edge must clear it rather than silently
//! inherit console scheduling, and the custom-pool case, where an action's own
//! pool must stay in force.

use super::{aggregate_gates, generate, generate_bundle, serial_graph};
use crate::ast::{CommandIsolation, Recipe};
use crate::ir::{Action, BuildEdge, BuildGraph, DependencyOrder, EdgeOutputVisibility};
use camino::{Utf8Path, Utf8PathBuf};
use proptest::prelude::*;
use test_support::ninja_edge::edge_pool_bindings;

/// Ninja's name for the console pool, as validation and rendering spell it.
const CONSOLE_POOL_NAME: &str = "console";

/// A custom pool a non-console action can select.
const CUSTOM_POOL: &str = "link_pool";

/// Return the exact edge-local bindings a console state must emit.
///
/// A console edge selects the pool, an edge that inherits a console action
/// pool without opting in clears it with an empty binding, and an edge whose
/// action carries no pool emits nothing at all.
fn expected_pool_bindings(console: bool, inherits_console_pool: bool) -> Vec<String> {
    match (console, inherits_console_pool) {
        (true, _) => vec!["pool = console".to_owned()],
        (false, true) => vec!["pool =".to_owned()],
        (false, false) => Vec::new(),
    }
}

/// Build the serial fixture with the supplied action pool.
fn console_graph(
    dependencies: Vec<Utf8PathBuf>,
    action_pool: Option<&str>,
) -> Result<BuildGraph, TestCaseError> {
    let mut graph = serial_graph(dependencies);
    graph
        .actions
        .get_mut("a")
        .ok_or_else(|| TestCaseError::fail("fixture action must exist"))?
        .pool = action_pool.map(str::to_owned);
    Ok(graph)
}

/// Generate a bundle whose parent and sibling edges share one action.
fn generate_console_bundle(
    selections: &[bool],
    parent_console: bool,
    action_pool: Option<&str>,
) -> Result<super::GeneratedNinja, TestCaseError> {
    let outputs: Vec<_> = (0..selections.len())
        .map(|index| Utf8PathBuf::from(format!("dep{index}")))
        .collect();
    let mut graph = console_graph(outputs.clone(), action_pool)?;
    let mut parent = graph
        .target_for_output(Utf8Path::new("aggregate"))
        .map(|(_, edge)| edge.clone())
        .ok_or_else(|| TestCaseError::fail("serial parent must exist"))?;
    parent.output_visibility = if parent_console {
        EdgeOutputVisibility::Immediate
    } else {
        EdgeOutputVisibility::Buffered
    };
    if !graph.replace_edge_for_output(Utf8Path::new("aggregate"), parent.clone()) {
        return Err(TestCaseError::fail("serial parent must be replaceable"));
    }

    for (output, selected) in outputs.iter().zip(selections) {
        let mut edge = parent.clone();
        edge.explicit_outputs = vec![output.clone()];
        edge.implicit_deps.clear();
        edge.dependency_order = DependencyOrder::Parallel;
        edge.output_visibility = if *selected {
            EdgeOutputVisibility::Immediate
        } else {
            EdgeOutputVisibility::Buffered
        };
        graph
            .insert_edge(edge)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
    }

    generate_bundle(&graph).map_err(|error| TestCaseError::fail(error.to_string()))
}

/// Build one parallel edge per output, all sharing a custom-pool action.
///
/// The direct path has no serial parent or synthetic gates, so it renders the
/// shared rule and its edges alone. That is what makes it a check of the rule
/// binding rather than of the bundle's staging.
fn custom_pool_direct_graph(count: usize) -> BuildGraph {
    let mut graph = BuildGraph::default();
    graph.actions.insert(
        "a".into(),
        Action {
            recipe: Recipe::Command {
                command: "touch out".into(),
                isolation: CommandIsolation::Shared,
            },
            description: None,
            depfile: None,
            deps_format: None,
            pool: Some(CUSTOM_POOL.into()),
            restat: false,
        },
    );
    for index in 0..count {
        assert!(
            graph
                .insert_edge(BuildEdge {
                    action_id: "a".into(),
                    inputs: Vec::new(),
                    implicit_deps: Vec::new(),
                    dependency_order: DependencyOrder::Parallel,
                    explicit_outputs: vec![Utf8PathBuf::from(format!("out{index}"))],
                    implicit_outputs: Vec::new(),
                    order_only_deps: Vec::new(),
                    progress_label: None,
                    phony: false,
                    always: false,
                    output_visibility: EdgeOutputVisibility::Buffered,
                })
                .is_ok(),
            "direct fixture outputs must be unique",
        );
    }
    graph
}

/// Assert every user, synthetic gate, and sidecar binding for one case.
fn assert_console_bindings(
    bundle: &super::GeneratedNinja,
    selections: &[bool],
    parent_console: bool,
    inherits_console_pool: bool,
) -> Result<(), TestCaseError> {
    let file = bundle.build_file();
    let mut selected_count = usize::from(parent_console);

    for (index, selected) in selections.iter().enumerate() {
        let output = format!("dep{index}");
        let bindings = edge_pool_bindings(file, &output)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(
            &bindings,
            &expected_pool_bindings(*selected, inherits_console_pool),
            "edge {} must carry exactly its own console binding",
            output
        );
        selected_count += usize::from(*selected);
    }

    let parent_bindings = edge_pool_bindings(file, "aggregate")
        .map_err(|error| TestCaseError::fail(error.to_string()))?;
    prop_assert_eq!(
        &parent_bindings,
        &expected_pool_bindings(parent_console, inherits_console_pool),
        "the serial parent must carry exactly its own console binding"
    );

    let gates =
        aggregate_gates(file).ok_or_else(|| TestCaseError::fail("aggregate gate list missing"))?;
    for gate in gates {
        let bindings = edge_pool_bindings(file, gate)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert!(
            bindings.is_empty(),
            "synthetic gate {gate} must carry no pool binding: {bindings:?}"
        );
    }

    prop_assert_eq!(bundle.console_edge_count(), selected_count);
    // A console action also renders one `pool = console` binding in its shared
    // rule block, so the whole file holds one binding per console edge plus
    // that single rule binding.
    let rule_bindings = usize::from(inherits_console_pool);
    prop_assert_eq!(
        file.lines()
            .filter(|line| *line == "  pool = console")
            .count(),
        selected_count + rule_bindings,
        "console bindings must be one per console edge plus the shared rule"
    );
    prop_assert!(
        bundle
            .dyndep_files()
            .iter()
            .all(|sidecar| !sidecar.content().contains("pool"))
    );
    Ok(())
}

/// Assert a custom action pool stays on the shared rule and off every edge.
///
/// The empty `pool =` binding exists only to clear an inherited *console*
/// pool. A custom pool is the action's own selection, so an edge that did not
/// opt into console scheduling must leave the rule's binding in force instead
/// of clearing it.
fn assert_custom_pool_survives(ninja: &str, outputs: &[String]) -> Result<(), TestCaseError> {
    let rule_binding = format!("  pool = {CUSTOM_POOL}");
    prop_assert_eq!(
        ninja.lines().filter(|line| *line == rule_binding).count(),
        1,
        "the shared rule must keep exactly one custom pool binding: {}",
        ninja
    );
    prop_assert!(
        !ninja.lines().any(|line| line == "  pool ="),
        "no edge may clear a custom pool with an empty binding: {ninja}"
    );
    for output in outputs {
        let bindings = edge_pool_bindings(ninja, output)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert!(
            bindings.is_empty(),
            "edge {output} must not carry a pool binding of its own: {bindings:?}"
        );
    }
    Ok(())
}

/// Generate sibling console selections that always cover both states.
///
/// Two fixed states lead the list so every case pairs a console sibling with a
/// non-console one, which is what distinguishes a selected binding from a
/// cleared one. The rest of the list stays arbitrary.
fn console_selections() -> impl Strategy<Value = Vec<bool>> {
    prop::collection::vec(any::<bool>(), 0..5).prop_map(|mut tail| {
        tail.insert(0, true);
        tail.insert(1, false);
        tail
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, .. ProptestConfig::default() })]

    #[test]
    fn console_bindings_match_user_edges(
        selections in console_selections(),
        parent_console in any::<bool>(),
    ) {
        let bundle = generate_console_bundle(&selections, parent_console, None)?;
        assert_console_bindings(&bundle, &selections, parent_console, false)?;
    }

    #[test]
    fn inherited_console_pool_is_cleared_on_non_console_edges(
        selections in console_selections(),
        parent_console in any::<bool>(),
    ) {
        let bundle = generate_console_bundle(
            &selections,
            parent_console,
            Some(CONSOLE_POOL_NAME),
        )?;
        assert_console_bindings(&bundle, &selections, parent_console, true)?;
    }

    #[test]
    fn a_custom_action_pool_survives_bundled_generation(count in 1usize..5) {
        let selections = vec![false; count];
        let bundle = generate_console_bundle(&selections, false, Some(CUSTOM_POOL))?;
        let mut outputs: Vec<String> = (0..count).map(|index| format!("dep{index}")).collect();
        outputs.push("aggregate".to_owned());
        assert_custom_pool_survives(bundle.build_file(), &outputs)?;
        prop_assert_eq!(bundle.console_edge_count(), 0);
        prop_assert!(
            bundle
                .dyndep_files()
                .iter()
                .all(|sidecar| !sidecar.content().contains("pool"))
        );
    }

    #[test]
    fn a_custom_action_pool_survives_direct_generation(count in 1usize..5) {
        let graph = custom_pool_direct_graph(count);
        let ninja = generate(&graph)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let outputs: Vec<String> = (0..count).map(|index| format!("out{index}")).collect();
        assert_custom_pool_survives(&ninja, &outputs)?;
    }
}
