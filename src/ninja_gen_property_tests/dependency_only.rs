//! Properties for dependency-only action and target lowering.

use super::super::{GeneratedNinja, generate, generate_bundle};
use camino::Utf8PathBuf;
use proptest::{prelude::*, test_runner::TestCaseError};

use crate::{
    ast::{Recipe, StringOrList},
    ir::{Action, BuildEdge, BuildGraph, DependencyOrder},
};

/// Build a graph containing a dependency-only action and its target edge.
fn dependency_only_graph(dependencies: &[String], order: DependencyOrder) -> BuildGraph {
    let mut graph = BuildGraph::default();
    graph.actions.insert(
        "aggregate".into(),
        Action {
            recipe: Recipe::Command {
                command: StringOrList::Empty,
            },
            description: None,
            depfile: None,
            deps_format: None,
            pool: None,
            restat: false,
        },
    );
    graph.targets.insert(
        Utf8PathBuf::from("all"),
        BuildEdge {
            action_id: "aggregate".into(),
            inputs: Vec::new(),
            implicit_deps: dependencies
                .iter()
                .map(|dependency| Utf8PathBuf::from(dependency.as_str()))
                .collect(),
            dependency_order: order,
            explicit_outputs: vec![Utf8PathBuf::from("all")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: false,
            always: false,
        },
    );
    graph
}

/// Extract dependencies from dyndep sidecars in their staged order.
fn staged_dependencies(bundle: &GeneratedNinja) -> Vec<String> {
    bundle
        .dyndep_files()
        .iter()
        .filter_map(|sidecar| {
            sidecar.content().lines().find_map(|line| {
                line.split_once(" | ")
                    .map(|(_, dependency)| dependency.into())
            })
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, .. ProptestConfig::default() })]

    #[test]
    fn dependency_only_actions_and_targets_lower_without_recipes(
        dependencies in prop::collection::vec("[a-z][a-z0-9_]{0,7}", 1..7)
    ) {
        let expected_parallel = format!("build all: phony | {}", dependencies.join(" "));
        let parallel = generate(&dependency_only_graph(
            &dependencies,
            DependencyOrder::Parallel,
        ))
        .map_err(|error| TestCaseError::fail(format!("parallel generation failed: {error}")))?;
        prop_assert!(parallel.contains(&expected_parallel));
        prop_assert!(!parallel.contains("rule aggregate"));
        prop_assert!(!parallel.contains("command ="));

        let serial = generate_bundle(&dependency_only_graph(
            &dependencies,
            DependencyOrder::Serial,
        ))
        .map_err(|error| TestCaseError::fail(format!("serial generation failed: {error}")))?;
        prop_assert!(!serial.build_file().contains("rule aggregate"));
        prop_assert!(!serial.build_file().contains("command ="));

        if dependencies.len() == 1 {
            prop_assert!(serial.build_file().contains(&expected_parallel));
            prop_assert!(serial.dyndep_files().is_empty());
        } else {
            prop_assert!(serial.build_file().contains("build all: phony | .netsuke/serial/"));
            prop_assert_eq!(staged_dependencies(&serial), dependencies);
        }
    }
}
