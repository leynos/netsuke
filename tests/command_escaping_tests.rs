#![allow(
    clippy::expect_used,
    reason = "command escaping tests use expect for diagnostics"
)]

//! Tests for shell quoting of command substitutions.

use netsuke::{ast::Recipe, ir::BuildGraph, manifest};
use rstest::rstest;

/// Prefix the provided YAML body with a required `netsuke_version`.
///
/// # Examples
/// ```
/// let y = manifest_yaml("targets: []");
/// assert!(y.starts_with("netsuke_version"));
/// ```
#[inline]
pub(crate) fn manifest_yaml(body: &str) -> String {
    format!("netsuke_version: 1.0.0\n{body}")
}

/// Extract shell words from the first target's command.
///
/// # Examples
/// ```
/// let words = command_words(
///     "targets:\n  - name: out\n    sources: in\n    command: \"echo hi\"\n",
/// );
/// assert_eq!(words, ["echo", "hi"]);
/// ```
fn command_words(body: &str) -> Vec<String> {
    let yaml = manifest_yaml(body);
    let manifest = manifest::from_str(&yaml).expect("parse");
    let graph = BuildGraph::from_manifest(&manifest).expect("graph");
    let action = graph.actions.values().next().expect("action");
    let Recipe::Command { command } = &action.recipe else {
        panic!("expected command");
    };
    shlex::split(command).expect("split command into words")
}

#[rstest]
fn inputs_and_outputs_are_quoted() {
    let words = command_words(
        "targets:\n  - name: 'out file'\n    sources: 'in file'\n    command: \"cat $in > $out\"\n",
    );
    assert_eq!(words, ["cat", "in file", ">", "out file"]);
}

#[rstest]
fn multiple_inputs_outputs_with_special_chars_are_quoted() {
    let words = command_words(
        "targets:\n  - name: ['out file', 'out&2']\n    sources: ['in file', 'input$1']\n    command: \"echo $in && echo $out\"\n",
    );
    assert_eq!(
        words,
        [
            "echo", "in file", "input$1", "&&", "echo", "out file", "out&2",
        ],
    );
}

#[rstest]
fn variable_name_overlap_not_rewritten() {
    let words = command_words(
        "targets:\n  - name: 'out file'\n    sources: in\n    command: \"echo $input > $out\"\n",
    );
    assert_eq!(words, ["echo", "$input", ">", "out file"]);
}

#[rstest]
fn output_variable_overlap_not_rewritten() {
    let words = command_words(
        "targets:\n  - name: out\n    sources: in\n    command: \"echo $output_dir > $out\"\n",
    );
    assert_eq!(words, ["echo", "$output_dir", ">", "out"]);
}

#[rstest]
fn newline_in_paths_is_quoted() {
    let words = command_words(
        "targets:\n  - name: \"o'ut\\nfile\"\n    sources: \"-in file\"\n    command: \"printf %s $in > $out\"\n",
    );
    assert_eq!(words, ["printf", "%s", "-in file", ">", "o'ut\nfile"]);
}

#[rstest]
fn command_without_placeholders_remains_valid() {
    let words =
        command_words("targets:\n  - name: out\n    sources: in\n    command: \"echo hi\"\n");
    assert_eq!(words, ["echo", "hi"]);
}

#[rstest]
#[case("echo \"unterminated")]
#[case("echo 'unterminated")]
#[case("echo `unterminated")]
fn invalid_command_errors(#[case] cmd: &str) {
    let escaped = cmd.replace('\\', "\\\\").replace('"', "\\\"");
    let yaml = manifest_yaml(&format!(
        "targets:\n  - name: out\n    sources: in\n    command: \"{escaped}\"\n"
    ));
    let manifest = manifest::from_str(&yaml).expect("parse");
    let err = BuildGraph::from_manifest(&manifest).expect_err("should fail");
    assert!(err.to_string().contains("not a valid shell command"));
}
