//! Verify bounded command-list target indexing and marker-key parity.

use crate::runner::process::test_fixtures::{
    graph_with_identity_action as shared_graph_with_identity_action, insert_edge,
};
use crate::{
    ast::{CommandIsolation, Recipe, StringOrList},
    ir::{Action, BuildEdge, BuildGraph, DependencyOrder},
    ninja_gen::{
        command_list::{COMMAND_LIST_FAILURE_PREFIX, action_identity},
        dyndep::generate_bundle_for_shell,
    },
    recipe_shell::RecipeShell,
    runner::process::command_list_targets::CommandListTargetIndex,
};
use camino::Utf8PathBuf;
use rstest::{fixture, rstest};

/// Build a graph with the shared action used by target-index tests.
#[fixture]
fn graph_with_identity_action() -> BuildGraph {
    shared_graph_with_identity_action()
}

#[rstest]
fn marker_fingerprint_matches_the_generator_key(mut graph_with_identity_action: BuildGraph) {
    insert_edge(
        &mut graph_with_identity_action,
        "identity",
        &["check-fmt"],
        false,
    );
    let graph = graph_with_identity_action;
    let index = CommandListTargetIndex::from_graph(&graph);
    let generated = generate_bundle_for_shell(&graph, RecipeShell::Posix)
        .expect("POSIX command list should generate");
    let fingerprint = action_identity("identity");
    let marker = format!("{COMMAND_LIST_FAILURE_PREFIX}{fingerprint}, entry 1");

    assert!(generated.build_file().contains(&marker));
    assert_eq!(
        index.clause(&fingerprint).as_deref(),
        Some("target \"check-fmt\"")
    );
}

#[rstest]
fn one_canonical_edge_with_multiple_outputs_is_one_candidate(
    mut graph_with_identity_action: BuildGraph,
) {
    insert_edge(
        &mut graph_with_identity_action,
        "identity",
        &["first", "second"],
        false,
    );
    let graph = graph_with_identity_action;
    let index = CommandListTargetIndex::from_graph(&graph);

    assert_eq!(
        index.clause(&action_identity("identity")).as_deref(),
        Some("target \"first\"")
    );
}

#[rstest]
fn implicit_output_is_used_when_an_edge_has_no_explicit_output(
    mut graph_with_identity_action: BuildGraph,
) {
    let result = graph_with_identity_action.insert_edge(BuildEdge {
        action_id: "identity".to_owned(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: Vec::new(),
        implicit_outputs: vec![Utf8PathBuf::from("implicit-target")],
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    });
    assert!(result.is_ok(), "test edge outputs should be unique");

    assert_eq!(
        CommandListTargetIndex::from_graph(&graph_with_identity_action)
            .clause(&action_identity("identity"))
            .as_deref(),
        Some("target \"implicit-target\"")
    );
}

#[rstest]
#[case::single_candidate(&["check-fmt"], Some("target \"check-fmt\""))]
#[case::ambiguous_candidates(
    &["lint", "check-fmt"],
    Some("targets \"check-fmt\", \"lint\"")
)]
#[case::without_outputs(&[], None)]
fn explicit_outputs_resolve_canonical_edge_candidates(
    mut graph_with_identity_action: BuildGraph,
    #[case] outputs: &[&str],
    #[case] expected_clause: Option<&str>,
) {
    for output in outputs {
        insert_edge(
            &mut graph_with_identity_action,
            "identity",
            &[*output],
            false,
        );
    }
    if outputs.is_empty() {
        insert_edge(&mut graph_with_identity_action, "identity", &[], false);
    }

    let index = CommandListTargetIndex::from_graph(&graph_with_identity_action);
    assert_eq!(
        index.clause(&action_identity("identity")).as_deref(),
        expected_clause
    );
}

#[rstest]
fn candidate_list_reports_omitted_names(mut graph_with_identity_action: BuildGraph) {
    for target in ["delta", "echo", "bravo", "charlie", "alpha"] {
        insert_edge(
            &mut graph_with_identity_action,
            "identity",
            &[target],
            false,
        );
    }
    let index = CommandListTargetIndex::from_graph(&graph_with_identity_action);

    assert_eq!(
        index.clause(&action_identity("identity")).as_deref(),
        Some("targets \"alpha\", \"bravo\", \"charlie\" and 2 more")
    );
}

#[rstest]
fn long_output_names_are_truncated_on_a_utf8_boundary(mut graph_with_identity_action: BuildGraph) {
    let target = format!("{}🚀🚀", "x".repeat(59));
    insert_edge(
        &mut graph_with_identity_action,
        "identity",
        &[&target],
        false,
    );
    let index = CommandListTargetIndex::from_graph(&graph_with_identity_action);
    let expected = format!("target \"{}…\"", "x".repeat(59));

    assert_eq!(
        index.clause(&action_identity("identity")).as_deref(),
        Some(expected.as_str())
    );
}

#[rstest]
fn escaped_output_names_stay_within_the_display_byte_limit(
    mut graph_with_identity_action: BuildGraph,
) {
    let target = format!("{}{}", "x".repeat(57), "\n".repeat(10));
    insert_edge(
        &mut graph_with_identity_action,
        "identity",
        &[&target],
        false,
    );
    let index = CommandListTargetIndex::from_graph(&graph_with_identity_action);
    let clause = index
        .clause(&action_identity("identity"))
        .expect("a command-list target should resolve");
    let display_name = clause
        .strip_prefix("target ")
        .expect("unique target clause should have its prefix");

    assert_eq!(display_name.len(), 64);
    assert_eq!(display_name, format!("\"{}\\n…\"", "x".repeat(57)));
}

#[rstest]
#[case::line_feed("line\nbreak", "target \"line\\nbreak\"")]
#[case::double_quote("double\"quote", "target \"double\\\"quote\"")]
#[case::apostrophe("single'quote", "target \"single'quote\"")]
fn output_names_escape_control_characters_and_quote_them(
    mut graph_with_identity_action: BuildGraph,
    #[case] target: &str,
    #[case] expected_clause: &str,
) {
    insert_edge(
        &mut graph_with_identity_action,
        "identity",
        &[target],
        false,
    );
    let index = CommandListTargetIndex::from_graph(&graph_with_identity_action);

    assert_eq!(
        index.clause(&action_identity("identity")).as_deref(),
        Some(expected_clause)
    );
}

#[rstest]
fn unknown_fingerprints_have_no_clause(mut graph_with_identity_action: BuildGraph) {
    insert_edge(
        &mut graph_with_identity_action,
        "identity",
        &["known"],
        false,
    );
    let index = CommandListTargetIndex::from_graph(&graph_with_identity_action);

    assert_eq!(index.clause(&"0".repeat(64)), None);
}

#[rstest]
fn uppercase_fingerprints_resolve_case_insensitively(mut graph_with_identity_action: BuildGraph) {
    insert_edge(
        &mut graph_with_identity_action,
        "identity",
        &["check-fmt"],
        false,
    );
    let index = CommandListTargetIndex::from_graph(&graph_with_identity_action);
    let fingerprint = action_identity("identity").to_ascii_uppercase();

    assert_eq!(
        index.clause(&fingerprint).as_deref(),
        Some("target \"check-fmt\"")
    );
}

#[rstest]
fn phony_and_dependency_only_edges_are_excluded(graph_with_identity_action: BuildGraph) {
    let mut graph = graph_with_identity_action;
    graph.actions.insert(
        "dependency-only".to_owned(),
        Action {
            recipe: Recipe::Command {
                command: StringOrList::Empty,
                isolation: CommandIsolation::Shared,
            },
            description: None,
            depfile: None,
            deps_format: None,
            pool: None,
            restat: false,
        },
    );
    insert_edge(&mut graph, "identity", &["aggregate"], true);
    insert_edge(&mut graph, "dependency-only", &["dependency"], false);

    assert_eq!(
        CommandListTargetIndex::from_graph(&graph).clause(&action_identity("identity")),
        None
    );
}
