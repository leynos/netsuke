//! Unit tests for Ninja file generation and rule synthesis.

use super::*;
use crate::ir::{Action, BuildEdge, BuildGraph};
use anyhow::{Result, ensure};
use rstest::rstest;

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

    let ninja = generate(&graph)?;
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
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("a".into(), action);
    graph.targets.insert(Utf8PathBuf::from("out"), edge);

    let ninja = generate(&graph)?;
    ensure!(ninja.contains("rule a"));
    ensure!(ninja.contains("command = /bin/sh -e -c"));
    ensure!(ninja.contains("echo '\"'\"'a b'\"'\"'"));
    ensure!(ninja.contains("\\\"\\$HOME\\\""));
    ensure!(ninja.contains("\\`whoami\\`"));
    ensure!(ninja.contains("printf %b"));
    ensure!(ninja.contains("\\n# line' | /bin/sh -e"));
    Ok(())
}

#[rstest]
fn generate_command_list_ninja_joins_a_fail_fast_chain() -> Result<()> {
    let action = Action {
        recipe: Recipe::Command {
            command: StringOrList::List(vec![
                "echo one".into(),
                "echo two".into(),
                "echo three".into(),
            ]),
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
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("a".into(), action);
    graph.targets.insert(Utf8PathBuf::from("out"), edge);

    let ninja = generate(&graph)?;
    ensure!(
        ninja.contains("command = echo one && echo two && echo three"),
        "command list should be joined into a fail-fast chain:\n{ninja}"
    );
    Ok(())
}

#[test]
fn assert_shell_command_tolerates_complex_syntax() {
    let command = r#"/bin/sh -c "echo 'nested quotes' && echo \"double\" && (echo subshell)""#;
    NamedAction::assert_shell_command(command);
}
