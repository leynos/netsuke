//! Tests for Jinja-templated manifest parsing.

use netsuke::{ast::Recipe, manifest};
use rstest::rstest;

fn manifest_yaml(body: &str) -> String {
    format!("netsuke_version: 1.0.0\n{body}")
}

#[rstest]
fn renders_global_vars() {
    let yaml = manifest_yaml(
        "vars:\n  who: world\ntargets:\n  - name: hello\n    command: echo {{ who }}\n",
    );

    let manifest = manifest::from_str(&yaml).expect("parse");
    let first = manifest.targets.first().expect("target");
    if let Recipe::Command { command } = &first.recipe {
        assert_eq!(command, "echo world");
    } else {
        panic!("Expected command recipe, got: {:?}", first.recipe);
    }
}

#[rstest]
fn undefined_variable_errors() {
    let yaml = manifest_yaml("targets:\n  - name: hello\n    command: echo {{ missing }}\n");

    assert!(manifest::from_str(&yaml).is_err());
}

#[rstest]
fn syntax_error_errors() {
    let yaml = manifest_yaml("targets:\n  - name: hello\n    command: echo {{ who\n");

    assert!(manifest::from_str(&yaml).is_err());
}

#[rstest]
#[case(true, "echo on")]
#[case(false, "echo off")]
fn renders_if_blocks(#[case] flag: bool, #[case] expected: &str) {
    let cmd = "{% if flag %}echo on{% else %}echo off{% endif %}";
    let yaml = manifest_yaml(&format!(
        concat!(
            "vars:\n",
            "  flag: {flag}\n",
            "targets:\n",
            "  - name: test\n",
            "    command: {cmd}\n",
        ),
        flag = flag,
        cmd = cmd,
    ));

    let manifest = manifest::from_str(&yaml).expect("parse");
    let first = manifest.targets.first().expect("target");
    if let Recipe::Command { command } = &first.recipe {
        assert_eq!(command, expected);
    } else {
        panic!("Expected command recipe, got: {:?}", first.recipe);
    }
}

#[rstest]
fn renders_for_loops() {
    let yaml = manifest_yaml(
        "vars:\n  items:\n    - a\n    - b\ntargets:\n{% for item in items %}\n  - name: \"{{ item }}\"\n    command: \"echo {{ item }}\"\n{% endfor %}\n",
    );

    let manifest = manifest::from_str(&yaml).expect("parse");
    assert_eq!(manifest.targets.len(), 2);
    let names: Vec<_> = manifest
        .targets
        .iter()
        .map(|t| match &t.name {
            netsuke::ast::StringOrList::String(s) => s.clone(),
            other => panic!("Expected String, got: {other:?}"),
        })
        .collect();
    assert_eq!(names, vec!["a", "b"]);

    let commands: Vec<_> = manifest
        .targets
        .iter()
        .map(|t| match &t.recipe {
            Recipe::Command { command } => command.clone(),
            other => panic!("Expected command recipe, got: {other:?}"),
        })
        .collect();
    assert_eq!(commands, vec!["echo a", "echo b"]);
}

#[rstest]
fn for_loop_non_iterable_errors() {
    let yaml = manifest_yaml(
        "vars:\n  items: 1\ntargets:\n{% for item in items %}\n  - name: \"{{ item }}\"\n    command: \"echo {{ item }}\"\n{% endfor %}\n",
    );

    assert!(manifest::from_str(&yaml).is_err());
}

#[rstest]
fn undefined_in_if_errors() {
    let yaml = manifest_yaml(
        "targets:\n  - name: test\n    command: {% if missing %}echo hi{% endif %}\n",
    );

    assert!(manifest::from_str(&yaml).is_err());
}

#[rstest]
fn undefined_in_for_errors() {
    let yaml = manifest_yaml(
        "targets:\n{% for item in missing %}\n  - name: \"{{ item }}\"\n    command: \"echo {{ item }}\"\n{% endfor %}\n",
    );

    assert!(manifest::from_str(&yaml).is_err());
}
