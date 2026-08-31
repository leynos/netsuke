//! Unit tests for Ninja file generation and rule synthesis.
use super::test_support::command_action;
use super::{
    NamedAction, NinjaGenError, RecipeShell, generate, generate_bundle, generate_into,
    generate_into_with_shell,
};
use crate::ast::{Recipe, StringOrList};
use crate::ir::{Action, BuildEdge, BuildGraph, DependencyOrder};
use anyhow::{Context, Result, bail, ensure};
use camino::Utf8PathBuf;
use rstest::rstest;

/// Build one action graph with the requested metadata field populated.
fn metadata_graph(field: &str, value: &str) -> Result<BuildGraph> {
    let mut action = command_action("true".into());
    match field {
        "description" => action.description = Some(value.into()),
        "depfile" => action.depfile = Some(value.into()),
        "deps_format" => action.deps_format = Some(value.into()),
        "pool" => action.pool = Some(value.into()),
        _ => bail!("test must use a known action metadata field: {field}"),
    }
    let mut graph = BuildGraph::default();
    graph.actions.insert("metadata".into(), action);
    Ok(graph)
}

/// Map an action metadata field name to its emitted Ninja binding key.
fn metadata_ninja_key(field: &str) -> Result<&str> {
    Ok(match field {
        "description" => "description",
        "depfile" => "depfile",
        "deps_format" => "deps",
        "pool" => "pool",
        _ => bail!("test must use a known action metadata field: {field}"),
    })
}

/// Assert every generator entry point rejects unsafe metadata without output.
fn assert_metadata_control_character_is_rejected(field: &str, value: &str) -> Result<()> {
    let graph = metadata_graph(field, value)?;
    let generate_error = generate(&graph).expect_err("unsafe metadata should not generate Ninja");
    ensure!(
        matches!(generate_error, NinjaGenError::UnsafeNinjaValue),
        "{field} should produce UnsafeNinjaValue, got {generate_error:?}"
    );

    let mut output = String::new();
    let generate_into_error = generate_into(&graph, &mut output)
        .expect_err("unsafe metadata should not write Ninja output");
    ensure!(
        matches!(generate_into_error, NinjaGenError::UnsafeNinjaValue),
        "{field} should produce UnsafeNinjaValue, got {generate_into_error:?}"
    );
    ensure!(
        output.is_empty(),
        "single-action metadata validation must not emit partial Ninja output: {output}"
    );

    let bundle_error =
        generate_bundle(&graph).expect_err("unsafe metadata should not bundle Ninja");
    ensure!(
        matches!(bundle_error, NinjaGenError::UnsafeNinjaValue),
        "{field} should produce UnsafeNinjaValue, got {bundle_error:?}"
    );
    Ok(())
}

#[rstest]
fn generate_simple_ninja() -> Result<()> {
    let action = Action {
        recipe: Recipe::Command {
            command: "echo hi".into(),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let edge = BuildEdge {
        action_id: "a".into(),
        inputs: vec![Utf8PathBuf::from("in")],
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("a".into(), action);
    graph.targets.insert(Utf8PathBuf::from("out"), edge);
    graph.default_targets.push(Utf8PathBuf::from("out"));

    let ninja = generate_posix(&graph)?;
    let expected = concat!(
        "rule a\n",
        "  command = echo hi\n\n",
        "build out: a in\n\n",
        "default out\n"
    );
    ensure!(
        ninja == expected,
        "expected Ninja manifest:\n{expected}\nactual:\n{ninja}"
    );
    Ok(())
}

#[test]
fn string_generation_apis_reject_reserved_paths() -> Result<()> {
    let action = command_action("true".into());
    let edge = BuildEdge {
        action_id: "reserved".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: vec![Utf8PathBuf::from(".netsuke/dyndep/reserved")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("reserved".into(), action);
    graph
        .targets
        .insert(Utf8PathBuf::from(".netsuke/dyndep/reserved"), edge);

    let generate_error = generate(&graph)
        .err()
        .context("generate must reject reserved paths")?;
    ensure!(
        matches!(generate_error, NinjaGenError::ReservedOutputPath { .. }),
        "unexpected generate error: {generate_error:?}"
    );

    let mut output = String::new();
    let generate_into_error = generate_into(&graph, &mut output)
        .err()
        .context("generate_into must reject reserved paths")?;
    ensure!(
        matches!(
            generate_into_error,
            NinjaGenError::ReservedOutputPath { .. }
        ),
        "unexpected generate_into error: {generate_into_error:?}"
    );
    ensure!(
        output.is_empty(),
        "reserved paths must not produce Ninja output"
    );
    Ok(())
}

/// Verify each supported metadata binding escapes literal dollars for Ninja output.
#[rstest]
#[case::description("description")]
#[case::depfile("depfile")]
#[case::deps_format("deps_format")]
#[case::pool("pool")]
fn metadata_values_escape_ninja_dollars(#[case] field: &str) -> Result<()> {
    let graph = metadata_graph(field, "literal$metadata")?;
    let key = metadata_ninja_key(field)?;
    let expected = format!("  {key} = literal$$metadata");

    let ninja = generate(&graph)?;
    ensure!(
        ninja.contains(&expected),
        "{field} must escape a literal dollar before Ninja parses it:\n{ninja}"
    );

    let bundle = generate_bundle(&graph)?;
    ensure!(
        bundle.build_file().contains(&expected),
        "{field} must be escaped in bundled Ninja output:\n{}",
        bundle.build_file()
    );
    Ok(())
}

/// Verify each supported metadata binding rejects newline, carriage-return, and NUL values.
#[rstest]
#[case::description("description")]
#[case::depfile("depfile")]
#[case::deps_format("deps_format")]
#[case::pool("pool")]
fn metadata_control_characters_are_rejected(#[case] field: &str) -> Result<()> {
    for (name, value) in [
        ("newline", "unsafe\nmetadata"),
        ("carriage return", "unsafe\rmetadata"),
        ("NUL", "unsafe\0metadata"),
    ] {
        assert_metadata_control_character_is_rejected(field, value)
            .with_context(|| format!("{field} must reject {name}"))?;
    }
    Ok(())
}

#[rstest]
fn generate_script_ninja_round_trips() -> Result<()> {
    let script = "echo 'a b' && echo \"$HOME\" && printf %s \"`whoami`\"\n# line";
    let action = Action {
        recipe: Recipe::Script {
            script: script.into(),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let edge = BuildEdge {
        action_id: "a".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("a".into(), action);
    graph.targets.insert(Utf8PathBuf::from("out"), edge);

    let ninja = generate_posix(&graph)?;
    ensure!(ninja.contains("rule a"));
    ensure!(ninja.contains("command = /bin/sh -e -c"));
    ensure!(ninja.contains(r"echo '\''a b'\''"));
    ensure!(ninja.contains("\\\"\\$$HOME\\\""));
    ensure!(ninja.contains("\\`whoami\\`"));
    ensure!(ninja.contains("printf %b"));
    ensure!(ninja.contains("\\n# line' | /bin/sh -e"));
    Ok(())
}

#[rstest]
fn generate_command_list_ninja_joins_a_fail_fast_chain() -> Result<()> {
    let action = command_action(StringOrList::List(vec![
        "echo one".into(),
        "echo two".into(),
        "echo three".into(),
    ]));
    let edge = BuildEdge {
        action_id: "a".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("a".into(), action);
    graph.targets.insert(Utf8PathBuf::from("out"), edge);

    let ninja = generate_posix(&graph)?;
    ensure!(
        ninja.contains("command = { _netsuke_background_before=$${!:-};"),
        "first list boundary should start the generated command:\n{ninja}"
    );
    ensure!(
        ninja.contains("if eval 'echo one'"),
        "first command should retain its evaluator:\n{ninja}"
    );
    ensure!(
        ninja.contains("if eval 'echo two'"),
        "second command should retain its evaluator:\n{ninja}"
    );
    ensure!(
        ninja.contains("if eval 'echo three'"),
        "third command should retain its evaluator:\n{ninja}"
    );
    ensure!(
        ninja.contains("if wait \"$$_netsuke_background_after\"; then :;"),
        "list boundary should wait for its one supported background job:\n{ninja}"
    );
    ensure!(
        ninja.matches("} && {").count() == 2,
        "three list boundaries should be joined by exactly two && operators:\n{ninja}"
    );
    Ok(())
}

#[rstest]
#[case::empty_list(StringOrList::List(Vec::new()))]
fn programmatic_empty_command_list_returns_a_typed_generation_error(#[case] command: StringOrList) {
    let action = command_action(command);
    let mut graph = BuildGraph::default();
    graph.actions.insert("empty".into(), action);

    let error = generate(&graph).expect_err("empty command recipe should not generate Ninja");
    assert!(
        matches!(error, NinjaGenError::EmptyCommandRecipe { action_index: 1 }),
        "empty command recipe should produce the stable typed error, got {error:?}"
    );
}
#[test]
fn nested_command_list_exec_returns_a_typed_generation_error() {
    let action = command_action(StringOrList::List(vec![
        "if true; then exec false; fi".into(),
    ]));
    let mut graph = BuildGraph::default();
    graph.actions.insert("nested-exec".into(), action);

    let error = generate_posix(&graph).expect_err("nested exec should not generate Ninja");
    assert!(
        matches!(
            error,
            NinjaGenError::UnsupportedCommandListExec {
                action_index: 1,
                entry_index: 1,
            }
        ),
        "nested exec should produce the stable typed error, got {error:?}"
    );
}

#[rstest]
#[case::dynamic_eval(
    "eval '$jobs'",
    NinjaGenError::UnanalyzableCommandListEval {
        action_index: 1,
        entry_index: 1,
    }
)]
#[case::newline(
    "echo safe\nbuild injected: phony",
    NinjaGenError::NinjaControlCharacter {
        action_index: 1,
        entry_index: 1,
    }
)]
fn unsafe_command_list_entries_return_typed_generation_errors(
    #[case] entry: &str,
    #[case] expected: NinjaGenError,
) {
    let action = command_action(StringOrList::List(vec![entry.into()]));
    let mut graph = BuildGraph::default();
    graph.actions.insert("unsafe".into(), action);
    let mut ninja = String::new();

    let error = generate_into_with_shell(&graph, &mut ninja, RecipeShell::Posix)
        .expect_err("unsafe command-list entries should not generate Ninja");
    assert!(
        matches!(
            (error, expected),
            (
                NinjaGenError::UnanalyzableCommandListEval {
                    action_index: 1,
                    entry_index: 1,
                },
                NinjaGenError::UnanalyzableCommandListEval { .. }
            ) | (
                NinjaGenError::NinjaControlCharacter {
                    action_index: 1,
                    entry_index: 1,
                },
                NinjaGenError::NinjaControlCharacter { .. }
            )
        ),
        "unsafe command-list entry should return its stable typed error"
    );
    assert!(
        ninja.is_empty(),
        "validation must reject the entry before it can inject Ninja output: {ninja}"
    );
}

#[test]
fn assert_shell_command_tolerates_complex_syntax() {
    let command = r#"/bin/sh -c "echo 'nested quotes' && echo \"double\" && (echo subshell)""#;
    NamedAction::assert_shell_command(command);
}
/// Generate Ninja text using the explicit POSIX compatibility renderer.
fn generate_posix(graph: &BuildGraph) -> Result<String, NinjaGenError> {
    let mut ninja = String::new();
    generate_into_with_shell(graph, &mut ninja, RecipeShell::Posix)?;
    Ok(ninja)
}

#[path = "ninja_gen_tests/power_shell.rs"]
mod power_shell;
