//! Check console scheduling across generated user and serial edges.

use super::{generate_bundle, serial_graph};
use crate::ir::DependencyOrder;
use camino::{Utf8Path, Utf8PathBuf};
use proptest::prelude::*;
use test_support::ninja_edge::edge_has_console_pool;

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, .. ProptestConfig::default() })]

    #[test]
    fn console_bindings_match_user_edges(
        selections in prop::collection::vec(any::<bool>(), 2..7),
        parent_console in any::<bool>(),
    ) {
        let outputs: Vec<_> = (0..selections.len())
            .map(|index| Utf8PathBuf::from(format!("dep{index}")))
            .collect();
        let mut graph = serial_graph(outputs.clone());
        let mut parent = graph
            .target_for_output(Utf8Path::new("aggregate"))
            .map(|(_, edge)| edge.clone())
            .ok_or_else(|| TestCaseError::fail("serial parent must exist"))?;
        parent.console = parent_console;
        prop_assert!(graph.replace_edge_for_output(Utf8Path::new("aggregate"), parent.clone()));

        for (output, selected) in outputs.iter().zip(&selections) {
            let mut edge = parent.clone();
            edge.explicit_outputs = vec![output.clone()];
            edge.implicit_deps.clear();
            edge.dependency_order = DependencyOrder::Parallel;
            edge.console = *selected;
            prop_assert!(graph.insert_edge(edge).is_ok());
        }

        let bundle = generate_bundle(&graph)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let file = bundle.build_file();
        for (output, selected) in outputs.iter().zip(&selections) {
            let actual = edge_has_console_pool(file, output.as_str())
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            prop_assert_eq!(actual, *selected);
        }
        let parent_actual = edge_has_console_pool(file, "aggregate")
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(parent_actual, parent_console);
        let selected_count = selections.iter().filter(|selected| **selected).count()
            + usize::from(parent_console);
        prop_assert_eq!(bundle.console_edge_count(), selected_count);
        prop_assert_eq!(
            file.lines().filter(|line| *line == "  pool = console").count(),
            selected_count
        );
        prop_assert!(
            bundle
                .dyndep_files()
                .iter()
                .all(|sidecar| !sidecar.content().contains("pool = console"))
        );
    }
}
