//! Unit tests for [`super::GraphView`]: node and edge classification,
//! canonical ordering, and golden renderer snapshots. Shared fixtures live in
//! `tests_support.rs`; property tests live in `tests_property.rs`.

use anyhow::Result;
use insta::assert_snapshot;
use rstest::rstest;

use crate::ir::BuildGraph;
use crate::snapshot_test_support::snapshot_settings;

use super::{EdgeClass, GraphView, NodeKind};

#[path = "tests_property.rs"]
mod property;
#[path = "tests_support.rs"]
mod support;

use support::{EdgeFixture, add_edge, make_action, p, render_dot, render_html};

#[test]
fn empty_graph_yields_empty_view() {
    let graph = BuildGraph::default();
    let view = GraphView::from_build_graph(&graph);
    assert!(view.nodes.is_empty());
    assert!(view.edges.is_empty());
    assert!(view.default_targets.is_empty());
    assert!(view.limit.is_none());
}

#[rstest]
#[case::single_input(&[("compile", &["src/a.c"][..], &["out/a.o"][..])])]
#[case::fan_in(&[
    ("link", &["out/a.o", "out/b.o"][..], &["out/app"][..]),
])]
#[case::fan_out(&[
    ("compile_a", &["src/a.c"][..], &["out/a.o"][..]),
    ("compile_b", &["src/b.c"][..], &["out/b.o"][..]),
])]
fn target_nodes_are_classified_as_targets(#[case] edges: &[(&str, &[&str], &[&str])]) {
    let mut graph = BuildGraph::default();
    for (id, _, _) in edges {
        graph
            .actions
            .insert((*id).into(), make_action(Some("desc")));
    }
    for (id, inputs, outputs) in edges {
        add_edge(
            &mut graph,
            EdgeFixture {
                action_id: id,
                inputs,
                explicit_outputs: outputs,
                ..EdgeFixture::default()
            },
        );
    }
    let view = GraphView::from_build_graph(&graph);
    let mut outputs: Vec<_> = edges
        .iter()
        .flat_map(|(_, _, outs)| outs.iter().copied())
        .collect();
    outputs.sort_unstable();
    outputs.dedup();
    for out in outputs {
        let node = view
            .nodes
            .iter()
            .find(|n| n.path == p(out))
            .expect("output node");
        assert!(matches!(node.kind, NodeKind::Target { .. }));
        assert_eq!(node.description.as_deref(), Some("desc"));
    }
}

#[test]
fn target_with_phony_flag_propagates_to_node_kind() {
    let mut graph = BuildGraph::default();
    graph
        .actions
        .insert("a".into(), make_action(Some("phony target")));
    add_edge(
        &mut graph,
        EdgeFixture {
            action_id: "a",
            explicit_outputs: &["out/phony"],
            phony: true,
            ..EdgeFixture::default()
        },
    );
    let view = GraphView::from_build_graph(&graph);
    let node = view
        .nodes
        .iter()
        .find(|n| n.path.as_str() == "out/phony")
        .expect("node must be present");
    assert_eq!(
        node.kind,
        NodeKind::Target {
            phony: true,
            always: false
        },
        "phony flag must propagate to NodeKind::Target"
    );
}

#[test]
fn target_with_always_flag_propagates_to_node_kind() {
    let mut graph = BuildGraph::default();
    graph
        .actions
        .insert("b".into(), make_action(Some("always target")));
    add_edge(
        &mut graph,
        EdgeFixture {
            action_id: "b",
            explicit_outputs: &["out/always"],
            always: true,
            ..EdgeFixture::default()
        },
    );
    let view = GraphView::from_build_graph(&graph);
    let node = view
        .nodes
        .iter()
        .find(|n| n.path.as_str() == "out/always")
        .expect("node must be present");
    assert_eq!(
        node.kind,
        NodeKind::Target {
            phony: false,
            always: true
        },
        "always flag must propagate to NodeKind::Target"
    );
}

#[test]
fn target_with_no_flags_yields_plain_target_kind() {
    let mut graph = BuildGraph::default();
    graph
        .actions
        .insert("c".into(), make_action(Some("plain target")));
    add_edge(
        &mut graph,
        EdgeFixture {
            action_id: "c",
            explicit_outputs: &["out/plain"],
            ..EdgeFixture::default()
        },
    );
    let view = GraphView::from_build_graph(&graph);
    let node = view
        .nodes
        .iter()
        .find(|n| n.path.as_str() == "out/plain")
        .expect("node must be present");
    assert_eq!(
        node.kind,
        NodeKind::Target {
            phony: false,
            always: false
        },
        "target with no flags must yield plain NodeKind::Target"
    );
}

#[test]
fn implicit_dep_yields_implicit_dep_edge() {
    let mut graph = BuildGraph::default();
    graph.actions.insert("a".into(), make_action(None));
    add_edge(
        &mut graph,
        EdgeFixture {
            action_id: "a",
            inputs: &["src/a.c"],
            implicit_deps: &["include/config.h"],
            explicit_outputs: &["out/a.o"],
            ..EdgeFixture::default()
        },
    );
    let view = GraphView::from_build_graph(&graph);
    let edge = view
        .edges
        .iter()
        .find(|e| e.from == p("include/config.h") && e.to == p("out/a.o"))
        .expect("implicit-dep edge");
    assert_eq!(edge.class, EdgeClass::ImplicitDep);
    let node = view
        .nodes
        .iter()
        .find(|n| n.path == p("include/config.h"))
        .expect("implicit-dep source node");
    assert_eq!(node.kind, NodeKind::Source);
}

#[test]
fn implicit_dep_emits_edge_to_every_output() {
    let mut graph = BuildGraph::default();
    graph.actions.insert("a".into(), make_action(None));
    add_edge(
        &mut graph,
        EdgeFixture {
            action_id: "a",
            inputs: &["src/a.c"],
            implicit_deps: &["include/config.h"],
            explicit_outputs: &["out/a.o"],
            implicit_outputs: &["out/a.d"],
            ..EdgeFixture::default()
        },
    );
    let view = GraphView::from_build_graph(&graph);
    let dep = p("include/config.h");
    let mut dep_targets: Vec<_> = view
        .edges
        .iter()
        .filter(|e| e.from == dep && e.class == EdgeClass::ImplicitDep)
        .map(|e| e.to.clone())
        .collect();
    dep_targets.sort();
    assert_eq!(dep_targets, vec![p("out/a.d"), p("out/a.o")]);
}

#[test]
fn order_only_dep_yields_order_only_edge() {
    let mut graph = BuildGraph::default();
    graph.actions.insert("a".into(), make_action(None));
    add_edge(
        &mut graph,
        EdgeFixture {
            action_id: "a",
            inputs: &["src/a.c"],
            explicit_outputs: &["out/a.o"],
            order_only_deps: &["build-dir"],
            ..EdgeFixture::default()
        },
    );
    let view = GraphView::from_build_graph(&graph);
    let edge = view
        .edges
        .iter()
        .find(|e| e.from == p("build-dir") && e.to == p("out/a.o"))
        .expect("order-only edge");
    assert_eq!(edge.class, EdgeClass::OrderOnly);
    let node = view
        .nodes
        .iter()
        .find(|n| n.path == p("build-dir"))
        .expect("order-only node");
    assert_eq!(node.kind, NodeKind::Source);
}

#[test]
fn implicit_output_yields_implicit_edge_class() {
    let mut graph = BuildGraph::default();
    graph.actions.insert("a".into(), make_action(None));
    add_edge(
        &mut graph,
        EdgeFixture {
            action_id: "a",
            inputs: &["src/a.c"],
            explicit_outputs: &["out/a.o"],
            implicit_outputs: &["out/a.d"],
            ..EdgeFixture::default()
        },
    );
    let view = GraphView::from_build_graph(&graph);
    let implicit = view
        .edges
        .iter()
        .find(|e| e.to == p("out/a.d"))
        .expect("edge to implicit output");
    assert_eq!(implicit.class, EdgeClass::ImplicitOutput);
    let explicit = view
        .edges
        .iter()
        .find(|e| e.to == p("out/a.o"))
        .expect("edge to explicit output");
    assert_eq!(explicit.class, EdgeClass::Explicit);
}

#[test]
fn edges_and_nodes_are_sorted_canonically() {
    let mut graph = BuildGraph::default();
    graph.actions.insert("a".into(), make_action(None));
    add_edge(
        &mut graph,
        EdgeFixture {
            action_id: "a",
            inputs: &["z", "a"],
            explicit_outputs: &["m"],
            ..EdgeFixture::default()
        },
    );
    let view = GraphView::from_build_graph(&graph);
    let paths: Vec<_> = view.nodes.iter().map(|n| n.path.clone()).collect();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted);
    let edge_keys: Vec<_> = view
        .edges
        .iter()
        .map(|e| (e.from.clone(), e.to.clone(), e.class))
        .collect();
    let mut sorted_edges = edge_keys.clone();
    sorted_edges.sort();
    assert_eq!(edge_keys, sorted_edges);
}

#[test]
fn default_targets_are_sorted_and_deduped() {
    let mut graph = BuildGraph::default();
    graph
        .default_targets
        .extend([p("z"), p("a"), p("a"), p("m")]);
    let view = GraphView::from_build_graph(&graph);
    assert_eq!(view.default_targets, vec![p("a"), p("m"), p("z")]);
}

fn golden_graph_view() -> GraphView {
    let mut graph = BuildGraph::default();
    graph
        .actions
        .insert("compile".into(), make_action(Some("compile app")));
    graph.default_targets.push(p("out/app"));
    add_edge(
        &mut graph,
        EdgeFixture {
            action_id: "compile",
            inputs: &["src/main.c"],
            implicit_deps: &["include/config.h"],
            explicit_outputs: &["out/app"],
            implicit_outputs: &["out/app.d"],
            order_only_deps: &["build"],
            ..EdgeFixture::default()
        },
    );
    GraphView::from_build_graph(&graph)
}

#[test]
fn golden_dot_output_matches_snapshot() -> Result<()> {
    let view = golden_graph_view();
    let dot = render_dot(&view)?;
    snapshot_settings("graph").bind(|| {
        assert_snapshot!("golden_dot", dot);
    });
    Ok(())
}

#[test]
fn golden_html_output_matches_snapshot() -> Result<()> {
    let view = golden_graph_view();
    let html = render_html(&view)?;
    snapshot_settings("graph").bind(|| {
        assert_snapshot!("golden_html", html);
    });
    Ok(())
}
