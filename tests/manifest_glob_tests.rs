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
    let msg = format!("{err:?}").to_lowercase();
    assert!(msg.contains("invalid glob pattern"), "{msg}");
}

#[rstest]
fn glob_returns_empty_when_no_matches() {
    let dir = tempfile::tempdir().expect("temp dir");
    let pattern = format!("{}/*.nomatch", dir.path().display());
    let yaml = manifest_yaml(&format!(
        concat!(
            "targets:\n",
            "  - foreach: glob('{pattern}')\n",
            "    name: none\n",
            "    command: echo hi\n",
        ),
        pattern = pattern,
    ));
    let manifest = manifest::from_str(&yaml).expect("parse");
    assert!(
        manifest.targets.is_empty(),
        "glob with no matches should yield no targets"
    );
}

#[rstest]
fn glob_does_not_cross_separator() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir(dir.path().join("sub")).expect("create subdir");
    std::fs::write(dir.path().join("sub").join("x.txt"), "x").expect("write file");
    let pattern = format!("{}/*.txt", dir.path().display());
    let yaml = manifest_yaml(&format!(
        concat!(
            "targets:\n",
            "  - foreach: glob('{pattern}')\n",
            "    name: bad\n",
            "    command: echo hi\n",
        ),
        pattern = pattern,
    ));
    let manifest = manifest::from_str(&yaml).expect("parse");
    assert!(manifest.targets.is_empty(), "wildcards must not cross '/'");
}

#[rstest]
fn glob_matches_dotfiles_with_wildcards() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join(".hidden.txt"), "h").expect("write file");
    let pattern = format!("{}/*.txt", dir.path().display());
    let yaml = manifest_yaml(&format!(
        concat!(
            "targets:\n",
            "  - foreach: glob('{pattern}')\n",
            "    name: ok\n",
            "    command: echo hi\n",
        ),
        pattern = pattern,
    ));
    let manifest = manifest::from_str(&yaml).expect("parse");
    assert_eq!(manifest.targets.len(), 1, "dotfiles should match");
}

#[rstest]
fn glob_is_case_sensitive() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("UPPER.TXT"), "x").expect("write file");
    let pattern = format!("{}/*.txt", dir.path().display());
    let yaml = manifest_yaml(&format!(
        concat!(
            "targets:\n",
            "  - foreach: glob('{pattern}')\n",
            "    name: fail\n",
            "    command: echo hi\n",
        ),
        pattern = pattern,
    ));
    let manifest = manifest::from_str(&yaml).expect("parse");
    assert!(manifest.targets.is_empty(), "glob should be case-sensitive");
}

#[rstest]
fn glob_accepts_windows_path_separators() {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::write(dir.path().join("a.txt"), "a").expect("write a");
    fs::write(dir.path().join("b.txt"), "b").expect("write b");
    let dir_win = dir.path().display().to_string().replace('/', "\\\\");
    let pattern = format!("{dir_win}\\\\*.txt");
    let yaml = manifest_yaml(&format!(
        concat!(
            "targets:\n",
            "  - foreach: glob('{pattern}')\n",
            "    name: \"{{{{ item | replace('{dir}/', '') | replace('.txt', '.out') }}}}\"\n",
            "    command: echo hi\n",
        ),
        pattern = pattern,
        dir = dir.path().display()
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
fn glob_filters_directories() {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::write(dir.path().join("a.txt"), "a").expect("write a");
    fs::create_dir(dir.path().join("sub")).expect("create subdir");
    let pattern = format!("{}/*", dir.path().display());
    let yaml = manifest_yaml(&format!(
        concat!(
            "targets:\n",
            "  - foreach: glob('{pattern}')\n",
            "    name: \"{{{{ item | replace('{dir}/', '') }}}}\"\n",
            "    command: echo hi\n",
        ),
        pattern = pattern,
        dir = dir.path().display()
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
    assert_eq!(names, vec!["a.txt"]);
}
