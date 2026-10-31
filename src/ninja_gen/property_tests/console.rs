//! Check console scheduling across generated user and serial edges.
//!
//! Each property generates a serial parent edge plus parallel sibling edges
//! and inspects the exact `pool` binding of every emitted statement. Coverage
//! includes the inherited case where the shared action already selects the
//! console pool, so a generated edge must clear it rather than silently
//! inherit console scheduling.

use super::{aggregate_gates, generate_bundle, serial_graph};
use crate::ir::{BuildGraph, DependencyOrder};
use camino::{Utf8Path, Utf8PathBuf};
use proptest::prelude::*;
use test_support::ninja_edge::edge_pool_bindings;

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

/// Build the serial fixture, optionally inheriting a console action pool.
fn console_graph(
    dependencies: Vec<Utf8PathBuf>,
    inherits_console_pool: bool,
) -> Result<BuildGraph, TestCaseError> {
    let mut graph = serial_graph(dependencies);
    if inherits_console_pool {
        graph
            .actions
            .get_mut("a")
            .ok_or_else(|| TestCaseError::fail("fixture action must exist"))?
            .pool = Some("console".into());
    }
    Ok(graph)
}

/// Generate a bundle whose parent and sibling edges share one action.
fn generate_console_bundle(
    selections: &[bool],
    parent_console: bool,
    inherits_console_pool: bool,
) -> Result<super::GeneratedNinja, TestCaseError> {
    let outputs: Vec<_> = (0..selections.len())
        .map(|index| Utf8PathBuf::from(format!("dep{index}")))
        .collect();
    let mut graph = console_graph(outputs.clone(), inherits_console_pool)?;
    let mut parent = graph
        .target_for_output(Utf8Path::new("aggregate"))
        .map(|(_, edge)| edge.clone())
        .ok_or_else(|| TestCaseError::fail("serial parent must exist"))?;
    parent.console = parent_console;
    if !graph.replace_edge_for_output(Utf8Path::new("aggregate"), parent.clone()) {
        return Err(TestCaseError::fail("serial parent must be replaceable"));
    }

    for (output, selected) in outputs.iter().zip(selections) {
        let mut edge = parent.clone();
        edge.explicit_outputs = vec![output.clone()];
        edge.implicit_deps.clear();
        edge.dependency_order = DependencyOrder::Parallel;
        edge.console = *selected;
        graph
            .insert_edge(edge)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
    }

    generate_bundle(&graph).map_err(|error| TestCaseError::fail(error.to_string()))
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
        let bundle = generate_console_bundle(&selections, parent_console, false)?;
        assert_console_bindings(&bundle, &selections, parent_console, false)?;
    }

    #[test]
    fn inherited_console_pool_is_cleared_on_non_console_edges(
        selections in console_selections(),
        parent_console in any::<bool>(),
    ) {
        let bundle = generate_console_bundle(&selections, parent_console, true)?;
        assert_console_bindings(&bundle, &selections, parent_console, true)?;
    }
}
