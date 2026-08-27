//! Property tests for Ninja edge formatting and serial dyndep lowering.
//!
//! The separator properties cover regular edge formatting. The serial
//! properties cover staged lowering, repeated dependencies, and determinism.

use proptest::prelude::*;
use test_support::ninja_gen::paths_strategy;

use super::{
    DisplayEdge, GeneratedDyndep, GeneratedNinja, NinjaGenError, generate, generate_bundle,
    test_support::command_action,
};
use crate::{
    ast::{Recipe, StringOrList},
    ir::{Action, BuildEdge, BuildGraph, DependencyOrder},
};

use camino::Utf8PathBuf;
use std::collections::{HashMap, HashSet};

#[path = "ninja_gen_property_tests/dependency_only.rs"]
mod dependency_only;
#[path = "ninja_gen_property_tests/ninja_oracle.rs"]
mod ninja_oracle;
use ninja_oracle::{NinjaCommandOracle, scalar_command_strategy, scalar_graph};
use proptest::test_runner::TestRunner;

fn edge_strategy_with_ranges(
    input_range: std::ops::Range<usize>,
    implicit_range: std::ops::Range<usize>,
    order_only_range: std::ops::Range<usize>,
) -> impl Strategy<Value = BuildEdge> {
    (
        "[a-z][a-z0-9_]{0,8}",
        paths_strategy("in", input_range),
        paths_strategy("out", 1..5),
        paths_strategy("iout", 0..5),
        paths_strategy("imp", implicit_range),
        paths_strategy("order", order_only_range),
    )
        .prop_map(
            |(
                action_id,
                inputs,
                explicit_outputs,
                implicit_outputs,
                implicit_deps,
                order_only_deps,
            )| {
                BuildEdge {
                    action_id,
                    inputs,
                    implicit_deps,
                    dependency_order: crate::ir::DependencyOrder::Parallel,
                    explicit_outputs,
                    implicit_outputs,
                    order_only_deps,
                    phony: false,
                    always: false,
                }
            },
        )
}

fn format_edge(edge: &BuildEdge) -> String {
    DisplayEdge {
        edge,
        action_name: &edge.action_id,
        action_restat: false,
        implicit_deps: &edge.implicit_deps,
    }
    .to_string()
}

fn build_line(formatted: &str) -> Option<&str> {
    formatted.lines().next()
}

fn dependency_side(line: &str) -> Option<&str> {
    line.split_once(": ").map(|(_, deps)| deps)
}

fn bare_pipe_position(line: &str) -> Option<usize> {
    line.match_indices(" | ").map(|(index, _)| index).next()
}

fn serial_graph(dependencies: Vec<Utf8PathBuf>) -> BuildGraph {
    let mut graph = BuildGraph::default();
    graph.actions.insert(
        "a".into(),
        Action {
            recipe: Recipe::Command {
                command: "touch aggregate".into(),
            },
            description: None,
            depfile: None,
            deps_format: None,
            pool: None,
            restat: false,
        },
    );
    graph.targets.insert(
        Utf8PathBuf::from("aggregate"),
        BuildEdge {
            action_id: "a".into(),
            inputs: Vec::new(),
            implicit_deps: dependencies,
            dependency_order: DependencyOrder::Serial,
            explicit_outputs: vec![Utf8PathBuf::from("aggregate")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: false,
            always: false,
        },
    );
    graph
}

fn generate_serial_bundle(dependencies: Vec<Utf8PathBuf>) -> Result<GeneratedNinja, TestCaseError> {
    generate_bundle(&serial_graph(dependencies))
        .map_err(|error| TestCaseError::fail(format!("serial bundle generation failed: {error}")))
}

fn aggregate_gates(build_file: &str) -> Option<Vec<&str>> {
    build_file.lines().find_map(|line| {
        line.strip_prefix("build aggregate: ")
            .and_then(|rest| rest.split_once(" | "))
            .map(|(_, gates)| gates.split_whitespace().collect())
    })
}

fn sidecar_dependency(content: &str) -> Option<&str> {
    content
        .lines()
        .find_map(|line| line.split_once(" | ").map(|(_, dependency)| dependency))
}

fn repeated_dependencies_strategy() -> impl Strategy<Value = Vec<Utf8PathBuf>> {
    ("[a-z][a-z0-9_]{0,7}", paths_strategy("dep", 0..4)).prop_map(|(repeated, mut dependencies)| {
        dependencies.push(Utf8PathBuf::from(&repeated));
        dependencies.push(Utf8PathBuf::from(repeated));
        dependencies
    })
}

/// Build the one-action graph used by command recipe generation properties.
fn command_graph(recipe: StringOrList) -> BuildGraph {
    let mut graph = BuildGraph::default();
    graph
        .actions
        .insert("action".into(), command_action(recipe));
    graph
}

fn command_list_graph(entries: &[String]) -> BuildGraph {
    command_graph(StringOrList::List(
        entries
            .iter()
            .map(|entry| format!("echo {entry}"))
            .collect(),
    ))
}

fn command_list_entry_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("plain"),
        Just("two words"),
        Just("apostrophe's"),
        Just("dollar$value"),
        Just("hash # comment"),
        Just("semi;colon"),
        Just("double\"quote"),
        Just("parentheses()"),
    ]
    .prop_map(str::to_owned)
}

fn canonical_shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''").replace('$', "$$"))
}

/// Verify real Ninja preserves scalar command text after backend escaping.
#[test]
fn scalar_command_output_matches_ninja_oracle() {
    let prepared_oracle =
        NinjaCommandOracle::try_create().expect("prepare real-Ninja command oracle");
    let Some(oracle) = prepared_oracle else {
        return;
    };
    let mut runner = TestRunner::new(ProptestConfig {
        cases: 128,
        ..ProptestConfig::default()
    });
    runner
        .run(&scalar_command_strategy(), |(command, braced_command)| {
            prop_assert!(
                braced_command.contains("${"),
                "braced property input must contain a shell braced expansion"
            );
            for candidate in [&command, &braced_command] {
                let ninja = generate(&scalar_graph(candidate.clone()))
                    .expect("scalar command should generate");
                let oracle_output = oracle.run_ninja_commands(&ninja)?;
                let observed = oracle_output
                    .strip_suffix("\r\n")
                    .or_else(|| oracle_output.strip_suffix('\n'));
                prop_assert_eq!(observed, Some(candidate.as_str()));
            }
            Ok(())
        })
        .expect("real-Ninja command oracle property should hold");
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, .. ProptestConfig::default() })]

    #[test]
    fn implicit_deps_separator_precedes_order_only_separator(edge in edge_strategy_with_ranges(1..5, 1..5, 1..5)) {
        let formatted = format_edge(&edge);
        let line = build_line(&formatted).expect("build line should be emitted");
        let deps = dependency_side(line).expect("build line should contain rule separator");
        let implicit_pos = bare_pipe_position(deps).expect("implicit separator should be emitted");
        let order_pos = deps.find(" || ").expect("order-only separator should be emitted");
        let (_, dependency_groups) = deps
            .split_once(' ')
            .expect("action identifier should precede dependencies");
        let (before_order_only, order_only) = dependency_groups
            .split_once(" || ")
            .expect("order-only separator should be emitted");
        let (inputs, implicit) = before_order_only
            .split_once(" | ")
            .expect("implicit separator should be emitted");

        prop_assert!(implicit_pos < order_pos);
        prop_assert_eq!(inputs.split_whitespace().collect::<Vec<_>>(), edge.inputs.iter().map(|path| path.as_str()).collect::<Vec<_>>());
        prop_assert_eq!(implicit.split_whitespace().collect::<Vec<_>>(), edge.implicit_deps.iter().map(|path| path.as_str()).collect::<Vec<_>>());
        prop_assert_eq!(order_only.split_whitespace().collect::<Vec<_>>(), edge.order_only_deps.iter().map(|path| path.as_str()).collect::<Vec<_>>());
    }

    #[test]
    fn implicit_deps_separator_is_absent_when_empty(edge in edge_strategy_with_ranges(0..5, 0..1, 1..5)) {
        let formatted = format_edge(&edge);
        let line = build_line(&formatted).expect("build line should be emitted");
        let deps = dependency_side(line).expect("build line should contain rule separator");

        prop_assert!(bare_pipe_position(deps).is_none());
        prop_assert!(deps.contains(" || "));
    }

    #[test]
    fn command_lists_preserve_order_boundaries_and_fail_fast_joins(entries in prop::collection::vec(command_list_entry_strategy(), 1..9)) {
        let ninja = generate(&command_list_graph(&entries)).expect("non-empty command list should generate");
        let command_line = ninja.lines().find(|line| line.starts_with("  command = "))
            .expect("generated action should include a command line");
        let mut previous = 0;
        for entry in &entries {
            let expected_entry = format!(
                "eval {}",
                canonical_shell_single_quote(&format!("echo {entry}")).replace('$', "$$")
            );
            let expected_count = entries.iter().filter(|candidate| *candidate == entry).count();
            prop_assert_eq!(
                command_line.matches(&expected_entry).count(),
                expected_count,
                "every entry should retain one independently quoted evaluator"
            );
            let position = command_line
                .get(previous..)
                .and_then(|remaining| remaining.find(&expected_entry))
                .expect("entries should retain their declaration order");
            previous += position + expected_entry.len();
        }
        prop_assert_eq!(
            command_line
                .matches("{ _netsuke_background_before=$${!:-};")
                .count(),
            entries.len()
        );
        prop_assert_eq!(command_line.matches("} && {").count(), entries.len() - 1);
    }

    #[test]
    fn programmatic_empty_command_lists_are_rejected(() in Just(())) {
        let graph = command_graph(StringOrList::List(Vec::new()));
        let error = generate(&graph).expect_err("empty command recipe should be rejected");
        let is_stable_empty_recipe_error = matches!(error, NinjaGenError::EmptyCommandRecipe { action_index: 1 });
        prop_assert!(is_stable_empty_recipe_error);
    }

    #[test]
    fn scalar_command_output_retains_the_preexisting_form(command in "echo [a-z]{1,12}") {
        let ninja = generate(&scalar_graph(command.clone())).expect("scalar command should generate");
        let expected_command_line = format!("  command = {command}\n");
        let retains_scalar_form = ninja.contains(&expected_command_line);
        let uses_list_boundary = ninja.contains("_netsuke_background_before=$${!:-}");
        prop_assert!(retains_scalar_form);
        prop_assert!(!uses_list_boundary);
    }

    #[test]
    fn short_serial_lists_need_no_staging(dependencies in paths_strategy("dep", 0..2)) {
        let bundle = generate_serial_bundle(dependencies.clone())?;
        prop_assert!(bundle.dyndep_files().is_empty());
        prop_assert!(!bundle.build_file().contains("ninja_required_version = 1.10"));
        for dependency in dependencies {
            prop_assert!(bundle.build_file().contains(dependency.as_str()));
        }
    }

    #[test]
    fn staged_serial_lists_preserve_declaration_order(dependencies in paths_strategy("dep", 2..6)) {
        let bundle = generate_serial_bundle(dependencies.clone())?;
        prop_assert!(bundle.build_file().contains("ninja_required_version = 1.10"));
        let gates = aggregate_gates(bundle.build_file())
            .ok_or_else(|| TestCaseError::fail("aggregate gate list missing"))?;
        prop_assert_eq!(gates.len(), dependencies.len());
        prop_assert_eq!(bundle.dyndep_files().len(), dependencies.len());

        for (index, ((_gate, dependency), sidecar)) in gates
            .iter()
            .zip(&dependencies)
            .zip(bundle.dyndep_files())
            .enumerate()
        {
            prop_assert_eq!(sidecar_dependency(sidecar.content()), Some(dependency.as_str()));
            let sidecar_line = format!("build {}: phony", sidecar.relative_path());
            let rendered_sidecar = bundle
                .build_file()
                .lines()
                .find(|line| line.starts_with(&sidecar_line))
                .ok_or_else(|| TestCaseError::fail(format!("sidecar stage {index} missing")))?;
            if let Some(previous) = index.checked_sub(1) {
                let previous_gate = gates
                    .get(previous)
                    .ok_or_else(|| TestCaseError::fail("preceding gate missing"))?;
                prop_assert!(rendered_sidecar.ends_with(previous_gate));
            }
            let binding = format!("  dyndep = {}", sidecar.relative_path());
            prop_assert!(bundle.build_file().contains(&binding));
        }
    }

    #[test]
    fn repeated_serial_dependencies_preserve_occurrences(
        dependencies in repeated_dependencies_strategy()
    ) {
        let bundle = generate_serial_bundle(dependencies.clone())?;
        let gates = aggregate_gates(bundle.build_file())
            .ok_or_else(|| TestCaseError::fail("aggregate gate list missing"))?;
        prop_assert_eq!(gates.len(), dependencies.len());
        let exposed = bundle
            .dyndep_files()
            .iter()
            .map(|sidecar| sidecar_dependency(sidecar.content()))
            .collect::<Vec<_>>();
        let expected = dependencies
            .iter()
            .map(|dependency| Some(dependency.as_str()))
            .collect::<Vec<_>>();
        prop_assert_eq!(exposed, expected);

        let unique_contents = bundle
            .dyndep_files()
            .iter()
            .map(GeneratedDyndep::content)
            .collect::<HashSet<_>>();
        prop_assert_eq!(bundle.dyndep_files().len(), unique_contents.len());
        let mut content_by_path = HashMap::new();
        for sidecar in bundle.dyndep_files() {
            if let Some(existing) = content_by_path.insert(sidecar.relative_path(), sidecar.content()) {
                prop_assert_eq!(existing, sidecar.content());
            }
        }
    }

    #[test]
    fn serial_bundle_generation_is_deterministic(dependencies in paths_strategy("dep", 0..6)) {
        let first = generate_serial_bundle(dependencies.clone())?;
        let second = generate_serial_bundle(dependencies)?;
        prop_assert_eq!(first.build_file(), second.build_file());
        let first_sidecars = first
            .dyndep_files()
            .iter()
            .map(|sidecar| (sidecar.relative_path(), sidecar.content()))
            .collect::<Vec<_>>();
        let second_sidecars = second
            .dyndep_files()
            .iter()
            .map(|sidecar| (sidecar.relative_path(), sidecar.content()))
            .collect::<Vec<_>>();
        prop_assert_eq!(first_sidecars, second_sidecars);
    }
}
