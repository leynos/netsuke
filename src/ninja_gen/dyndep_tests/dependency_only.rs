//! Regression coverage for dependency-only actions with serial dependencies.

use crate::ast::Recipe;
use crate::ir::{Action, BuildEdge, BuildGraph, DependencyOrder};
use crate::ninja_gen::dyndep::generate_bundle;
use anyhow::{Result, ensure};
use camino::Utf8PathBuf;

#[test]
fn dependency_only_serial_bundle_uses_phony_without_a_recipe() -> Result<()> {
    let mut graph = BuildGraph::default();
    graph.actions.insert(
        "aggregate".into(),
        Action {
            recipe: Recipe::DependencyOnly,
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
            implicit_deps: vec!["check-fmt".into(), "lint".into()],
            dependency_order: DependencyOrder::Serial,
            explicit_outputs: vec![Utf8PathBuf::from("all")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: true,
            always: false,
        },
    );

    let bundle = generate_bundle(&graph)?;
    ensure!(
        bundle
            .build_file()
            .contains("build all: phony | .netsuke/serial/"),
        "dependency-only serial aggregate should use staged phony dependencies: {}",
        bundle.build_file()
    );
    ensure!(
        !bundle.build_file().contains("rule aggregate")
            && !bundle.build_file().contains("command ="),
        "dependency-only serial aggregate must not emit a synthetic recipe: {}",
        bundle.build_file()
    );
    Ok(())
}
