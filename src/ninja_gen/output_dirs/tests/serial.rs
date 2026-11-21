//! Preparation ordering for a recipe whose dependencies run serially.
//!
//! A staged serial recipe's raw dependency list is revealed late through the
//! dyndep sidecars, so naming it on the preparation edge would expose it early.
//! These cases pin that the gates stand in its place.

use super::{EdgeShape, insert_edge, line_starting_with, prerequisites};
use crate::ir::BuildGraph;
use crate::ninja_gen::generate_bundle;
use crate::ninja_gen::test_support::command_action;
use camino::Utf8Path;

/// A staged serial recipe prepares its directory through the gates.
///
/// The raw serial list is revealed late through the dyndep sidecars so Ninja
/// cannot start a later stage early. Naming that list directly on the
/// preparation edge would expose it, so the gates must stand in its place.
#[test]
fn a_serial_recipe_prepares_after_its_gates() {
    let mut graph = BuildGraph::default();
    graph
        .actions
        .insert("run".to_owned(), command_action("true".into()));
    insert_edge(
        &mut graph,
        "run",
        EdgeShape {
            outputs: &[Utf8Path::new("build/report")],
            implicit_deps: &[Utf8Path::new("setup"), Utf8Path::new("check")],
            serial: true,
            phony: true,
            ..EdgeShape::default()
        },
    );
    let bundle = generate_bundle(&graph).expect("the fixture should generate");
    let edge = line_starting_with(bundle.build_file(), "build .netsuke/phony/dir-");
    let prerequisites = prerequisites(edge);
    assert_eq!(
        prerequisites.len(),
        2,
        "the preparation edge must wait on one gate per stage: {edge}",
    );
    assert!(
        prerequisites
            .iter()
            .all(|path| path.starts_with(".netsuke/serial/")),
        "preparation must name the staged gates, not the raw dependencies: {edge}",
    );
}
