//! Property tests for [`crate::graph_view::GraphView`]: the projected view
//! and its renderings must be invariant under graph insertion order.

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

use crate::graph_view::GraphView;
use crate::ir::BuildGraph;

use super::support::{EdgeFixture, add_edge, make_action, render_dot, render_html};

#[derive(Debug, Clone)]
struct EdgeSpec {
    action_id: String,
    inputs: Vec<String>,
    implicit_deps: Vec<String>,
    explicit_outputs: Vec<String>,
    implicit_outputs: Vec<String>,
    order_only_deps: Vec<String>,
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

/// Convert a fallible render outcome into a proptest case failure.
fn render_failure(err: &anyhow::Error) -> TestCaseError {
    TestCaseError::fail(err.to_string())
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
        let dot_a = render_dot(&view_a).map_err(|err| render_failure(&err))?;
        let dot_b = render_dot(&view_b).map_err(|err| render_failure(&err))?;
        prop_assert_eq!(dot_a, dot_b);
        let html_a = render_html(&view_a).map_err(|err| render_failure(&err))?;
        let html_b = render_html(&view_b).map_err(|err| render_failure(&err))?;
        prop_assert_eq!(html_a, html_b);
    }
}
