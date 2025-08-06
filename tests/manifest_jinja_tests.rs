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
