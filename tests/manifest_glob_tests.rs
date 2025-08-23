//! Tests for file globbing via the `glob()` Jinja helper.

use netsuke::{ast::StringOrList, manifest};
use rstest::rstest;
use std::fs;

fn manifest_yaml(body: &str) -> String {
    format!("netsuke_version: 1.0.0\n{body}")
}

#[rstest]
fn glob_expands_sorted_matches() {
    let dir = tempfile::tempdir().expect("temp dir");
    let b = dir.path().join("b.txt");
    let a = dir.path().join("a.txt");
    fs::write(&b, "b").expect("write b");
    fs::write(&a, "a").expect("write a");
    let dir_str = dir.path().display().to_string();
    let pattern = format!("{dir_str}/*.txt");
    let yaml = manifest_yaml(&format!(
        concat!(
            "targets:\n",
            "  - foreach: glob('{pattern}')\n",
            "    name: \"{{{{ item | replace('{dir}/', '') | replace('.txt', '.out') }}}}\"\n",
            "    command: echo hi\n",
        ),
        pattern = pattern,
        dir = dir_str
    ));
    let manifest = manifest::from_str(&yaml).expect("parse");
    let names: Vec<_> = manifest
        .targets
        .iter()
        .map(|t| match &t.name {
            StringOrList::String(s) => s.clone(),
            other => panic!("expected String, got {other:?}"),
        })
        .collect();
    assert_eq!(names, vec!["a.out", "b.out"]);
}

#[rstest]
fn glob_invalid_pattern_errors() {
    let yaml =
        manifest_yaml("targets:\n  - foreach: glob('[')\n    name: bad\n    command: echo hi\n");
    let err = manifest::from_str(&yaml).expect_err("invalid pattern should error");
    assert!(
        err.chain().any(|e| e
            .to_string()
            .to_lowercase()
            .contains("invalid glob pattern")),
        "unexpected error: {err}"
    );
}
