//! Tests for shell quoting of command substitutions.
use netsuke::{ast::Recipe, ir::BuildGraph, manifest};
use rstest::rstest;

fn manifest_yaml(body: &str) -> String {
    format!("netsuke_version: 1.0.0\n{body}")
}

#[rstest]
fn inputs_and_outputs_are_quoted() {
    let yaml = manifest_yaml(
        "targets:\n  - name: 'out file'\n    sources: 'in file'\n    command: \"cat $in > $out\"\n",
    );
    let manifest = manifest::from_str(&yaml).expect("parse");
    let graph = BuildGraph::from_manifest(&manifest).expect("graph");
    let action = graph.actions.values().next().expect("action");
    let Recipe::Command { command } = &action.recipe else {
        panic!("expected command")
    };
    assert_eq!(command, "cat in' file' > out' file'");
}

#[rstest]
fn multiple_inputs_outputs_with_special_chars_are_quoted() {
    let yaml = manifest_yaml(
        "targets:\n  - name: ['out file', 'out&2']\n    sources: ['in file', 'input$1']\n    command: \"echo $in && echo $out\"\n",
    );
    let manifest = manifest::from_str(&yaml).expect("parse");
    let graph = BuildGraph::from_manifest(&manifest).expect("graph");
    let action = graph.actions.values().next().expect("action");
    let Recipe::Command { command } = &action.recipe else {
        panic!("expected command")
    };
    assert_eq!(
        command,
        "echo in' file' input'$1' && echo out' file' out'&2'",
    );
}

#[rstest]
fn invalid_command_errors() {
    let yaml = manifest_yaml(
        "targets:\n  - name: out\n    sources: in\n    command: \"echo 'unterminated\"\n",
    );
    let manifest = manifest::from_str(&yaml).expect("parse");
    let err = BuildGraph::from_manifest(&manifest).expect_err("should fail");
    assert!(err.to_string().contains("not a valid shell command"));
}
