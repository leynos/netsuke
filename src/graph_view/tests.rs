//! Unit and property tests for [`super::GraphView`].

use camino::Utf8PathBuf;
use insta::assert_snapshot;
use proptest::prelude::*;
use rstest::rstest;

use crate::ast::Recipe;
use crate::graph_view::render::GraphRenderer;
use crate::graph_view::render_dot::DotRenderer;
use crate::graph_view::render_html::HtmlRenderer;
use crate::ir::{Action, BuildEdge, BuildGraph};
use crate::snapshot_test_support::snapshot_settings;

use super::{EdgeClass, GraphView, NodeKind};

fn make_action(description: Option<&str>) -> Action {
    Action {
        recipe: Recipe::Command {
            command: "echo".into(),
        },
        description: description.map(str::to_owned),
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    }
}

fn p(s: &str) -> Utf8PathBuf {
    Utf8PathBuf::from(s)
}

fn render_dot(view: &GraphView) -> String {
    let mut out = Vec::new();
    DotRenderer::new()
        .render(view, &mut out)
        .expect("render DOT graph");
    String::from_utf8(out).expect("DOT renderer emits UTF-8")
}

fn render_html(view: &GraphView) -> String {
    let mut out = Vec::new();
    HtmlRenderer::new(Some("en-US"))
        .render(view, &mut out)
        .expect("render HTML graph");
    String::from_utf8(out).expect("HTML renderer emits UTF-8")
}

#[derive(Default, Clone, Copy)]
struct EdgeFixture<'a> {
    action_id: &'a str,
    inputs: &'a [&'a str],
    implicit_deps: &'a [&'a str],
    explicit_outputs: &'a [&'a str],
    implicit_outputs: &'a [&'a str],
    order_only_deps: &'a [&'a str],
    phony: bool,
    always: bool,
}

fn add_edge(graph: &mut BuildGraph, fixture: EdgeFixture<'_>) {
    let edge = BuildEdge {
        action_id: fixture.action_id.into(),
        inputs: fixture.inputs.iter().map(|s| p(s)).collect(),
        implicit_deps: fixture.implicit_deps.iter().map(|s| p(s)).collect(),
        explicit_outputs: fixture.explicit_outputs.iter().map(|s| p(s)).collect(),
        implicit_outputs: fixture.implicit_outputs.iter().map(|s| p(s)).collect(),
        order_only_deps: fixture.order_only_deps.iter().map(|s| p(s)).collect(),
        phony: fixture.phony,
        always: fixture.always,
    };
    for out in &edge.explicit_outputs {
        graph.targets.insert(out.clone(), edge.clone());
    }
    for out in &edge.implicit_outputs {
        graph.targets.insert(out.clone(), edge.clone());
    }
}

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
fn golden_dot_output_matches_snapshot() {
    let view = golden_graph_view();
    snapshot_settings("graph").bind(|| {
        assert_snapshot!("golden_dot", render_dot(&view));
    });
}

#[test]
fn golden_html_output_matches_snapshot() {
    let view = golden_graph_view();
    snapshot_settings("graph").bind(|| {
        assert_snapshot!("golden_html", render_html(&view));
    });
}

fn build_graph_from_edge_specs(
    actions: &[(String, Option<String>)],
    edge_specs: &[EdgeSpec],
) -> BuildGraph {
    let mut graph = BuildGraph::default();
    for (id, desc) in actions {
        graph
            .actions
            .insert(id.clone(), make_action(desc.as_deref()));
    }
    for spec in edge_specs {
        let inputs: Vec<&str> = spec.inputs.iter().map(String::as_str).collect();
        let implicit_deps: Vec<&str> = spec.implicit_deps.iter().map(String::as_str).collect();
        let explicit_outputs: Vec<&str> =
            spec.explicit_outputs.iter().map(String::as_str).collect();
        let implicit_outputs: Vec<&str> =
            spec.implicit_outputs.iter().map(String::as_str).collect();
        let order_only_deps: Vec<&str> = spec.order_only_deps.iter().map(String::as_str).collect();
        add_edge(
            &mut graph,
            EdgeFixture {
                action_id: &spec.action_id,
                inputs: &inputs,
                implicit_deps: &implicit_deps,
                explicit_outputs: &explicit_outputs,
                implicit_outputs: &implicit_outputs,
                order_only_deps: &order_only_deps,
                phony: false,
                always: false,
            },
        );
    }
    graph
}

#[derive(Debug, Clone)]
struct EdgeSpec {
    action_id: String,
    inputs: Vec<String>,
    implicit_deps: Vec<String>,
    explicit_outputs: Vec<String>,
    implicit_outputs: Vec<String>,
    order_only_deps: Vec<String>,
}

fn arb_path() -> impl Strategy<Value = String> {
    "[a-d]{1,3}".prop_map(String::from)
}

fn arb_edge_spec(action_id: String) -> impl Strategy<Value = EdgeSpec> {
    (
        prop::collection::vec(arb_path(), 0..3),
        prop::collection::vec(arb_path(), 0..2),
        prop::collection::vec(arb_path(), 1..3),
        prop::collection::vec(arb_path(), 0..2),
        prop::collection::vec(arb_path(), 0..2),
    )
        .prop_map(
            move |(inputs, implicit_deps, explicit_outputs, implicit_outputs, order_only_deps)| {
                EdgeSpec {
                    action_id: action_id.clone(),
                    inputs,
                    implicit_deps,
                    explicit_outputs,
                    implicit_outputs,
                    order_only_deps,
                }
            },
        )
}

fn arb_graph_inputs() -> impl Strategy<Value = (Vec<(String, Option<String>)>, Vec<EdgeSpec>)> {
    prop::collection::vec(0u8..4, 1..4).prop_flat_map(|action_ids| {
        let actions: Vec<(String, Option<String>)> = action_ids
            .into_iter()
            .enumerate()
            .map(|(i, _)| (format!("a{i}"), Some(format!("desc-{i}"))))
            .collect();
        let edge_strategies: Vec<_> = actions
            .iter()
            .map(|(id, _)| arb_edge_spec(id.clone()))
            .collect();
        let actions_clone = actions.clone();
        edge_strategies.prop_map(move |edges| (actions_clone.clone(), edges))
    })
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        .. ProptestConfig::default()
    })]

    /// Property: `GraphView` is invariant under non-deterministic insertion
    /// order. The same logical graph projected through two distinct
    /// construction sequences must produce equal views.
    #[test]
    fn graphview_is_insertion_order_invariant(
        (actions, raw_edges) in arb_graph_inputs(),
    ) {
        // `BuildGraph::targets` is keyed by output path: any two edges
        // sharing an output collide at insertion time, which `from_manifest`
        // rejects as `DuplicateOutput`. Filter the generator's input space
        // to the realistic case where outputs are globally disjoint.
        let mut owned_outputs = std::collections::BTreeSet::new();
        let edges: Vec<_> = raw_edges
            .into_iter()
            .filter(|e| {
                let mut all = e.explicit_outputs.clone();
                all.extend(e.implicit_outputs.iter().cloned());
                if all.iter().any(|o| owned_outputs.contains(o)) {
                    return false;
                }
                for o in &all {
                    owned_outputs.insert(o.clone());
                }
                true
            })
            .collect();

        let mut reversed_actions = actions.clone();
        reversed_actions.reverse();
        let mut reversed_edges = edges.clone();
        reversed_edges.reverse();

        // Each call constructs fresh `HashMap`s with independent
        // `RandomState` seeds; combined with the reversed insertion order,
        // any leak of iteration ordering into `GraphView` shows up here.
        let g_forward = build_graph_from_edge_specs(&actions, &edges);
        let g_reversed = build_graph_from_edge_specs(&reversed_actions, &reversed_edges);

        let view_a = GraphView::from_build_graph(&g_forward);
        let view_b = GraphView::from_build_graph(&g_reversed);
        prop_assert_eq!(&view_a, &view_b);
        prop_assert_eq!(render_dot(&view_a), render_dot(&view_b));
        prop_assert_eq!(render_html(&view_a), render_html(&view_b));
    }
}
