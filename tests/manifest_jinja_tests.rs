//! Tests for Jinja-templated manifest parsing.

use netsuke::{ast::Recipe, manifest};
use rstest::rstest;

#[rstest]
fn renders_global_vars() {
    let yaml = r"
netsuke_version: 1.0.0
vars:
  who: world
targets:
  - name: hello
    command: echo {{ who }}
";

    let manifest = manifest::from_str(yaml).expect("parse");
    let first = manifest.targets.first().expect("target");
    if let Recipe::Command { command } = &first.recipe {
        assert_eq!(command, "echo world");
    } else {
        panic!("Expected command recipe, got: {:?}", first.recipe);
    }
}

#[rstest]
fn undefined_variable_errors() {
    let yaml = r"
netsuke_version: 1.0.0
targets:
  - name: hello
    command: echo {{ missing }}
";

    assert!(manifest::from_str(yaml).is_err());
}

#[rstest]
fn syntax_error_errors() {
    let yaml = r"
netsuke_version: 1.0.0
targets:
  - name: hello
    command: echo {{ who
";

    assert!(manifest::from_str(yaml).is_err());
}

#[rstest]
#[case(true, "echo on")]
#[case(false, "echo off")]
fn renders_if_blocks(#[case] flag: bool, #[case] expected: &str) {
    let yaml = format!(
        concat!(
            "netsuke_version: 1.0.0\n",
            "vars:\n",
            "  flag: {flag}\n",
            "targets:\n",
            "  - name: test\n",
            "    command: {{% if flag %}}echo on{{% else %}}echo off{{% endif %}}\n"
        ),
        flag = flag
    );

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
    let yaml = r#"
netsuke_version: 1.0.0
vars:
  items:
    - a
    - b
targets:
{% for item in items %}
  - name: "{{ item }}"
    command: "echo {{ item }}"
{% endfor %}
"#;

    let manifest = manifest::from_str(yaml).expect("parse");
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
}

#[rstest]
fn for_loop_non_iterable_errors() {
    let yaml = r#"
netsuke_version: 1.0.0
vars:
  items: 1
targets:
{% for item in items %}
  - name: "{{ item }}"
    command: "echo {{ item }}"
{% endfor %}
"#;

    assert!(manifest::from_str(yaml).is_err());
}
