//! Tests for file globbing via the `glob()` Jinja helper.

use netsuke::{
    ast::{NetsukeManifest, StringOrList},
    manifest,
};
use rstest::{fixture, rstest};
use std::{fs, path::Path};

fn manifest_yaml(body: &str) -> String {
    format!("netsuke_version: 1.0.0\n{body}")
}

fn target_names(manifest: &NetsukeManifest) -> Vec<String> {
    manifest
        .targets
        .iter()
        .map(|t| match &t.name {
            StringOrList::String(s) => s.clone(),
            other => panic!("expected String, got {other:?}"),
        })
        .collect()
}

fn create_test_files(base: &Path, files: &[(&str, &str)]) {
    for (rel, content) in files {
        let path = base.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(path, content).expect("write file");
    }
}

fn create_test_dirs(base: &Path, dirs: &[&str]) {
    for d in dirs {
        fs::create_dir_all(base.join(d)).expect("create dir");
    }
}

#[fixture]
fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("temp dir")
}

#[rstest]
#[case(
    &[("b.txt", "b"), ("a.txt", "a")],
    &[],
    "*.txt",
    "{{ item | replace('{dir}/', '') | replace('.txt', '.out') }}",
    &["a.out", "b.out"],
    "expands and sorts matches",
)]
#[case(
    &[],
    &[],
    "*.nomatch",
    "none",
    &[],
    "no targets when pattern has no matches",
)]
#[case(
    &[("sub/x.txt", "x")],
    &[],
    "*.txt",
    "bad",
    &[],
    "wildcards must not cross '/'",
)]
#[case(
    &[("sub/x.txt", "x")],
    &[],
    "**/*.txt",
    "{{ item | replace('{dir}/', '') }}",
    &["sub/x.txt"],
    "** spans directories",
)]
#[case(
    &[(".hidden.txt", "h")],
    &[],
    "*.txt",
    "{{ item }}",
    &[".hidden.txt"],
    "dotfiles should match",
)]
#[case(
    &[("UPPER.TXT", "x")],
    &[],
    "*.txt",
    "fail",
    &[],
    "glob should be case-sensitive",
)]
#[case(
    &[("a.txt", "a")],
    &["sub"],
    "*",
    "{{ item }}",
    &["a.txt"],
    "should filter directories",
)]
fn test_glob_behavior(
    temp_dir: tempfile::TempDir,
    #[case] files: &[(&str, &str)],
    #[case] dirs: &[&str],
    #[case] pattern_suffix: &str,
    #[case] name_template: &str,
    #[case] expected_partial: &[&str],
    #[case] description: &str,
) {
    create_test_files(temp_dir.path(), files);
    create_test_dirs(temp_dir.path(), dirs);

    let dir_str = temp_dir.path().display().to_string();
    let dir_fwd = dir_str.replace('\\', "/");
    let pattern = format!("{dir_str}/{pattern_suffix}");
    let yaml = manifest_yaml(&format!(
        concat!(
            "targets:\n",
            "  - foreach: glob('{pattern}')\n",
            "    name: \"{name_template}\"\n",
            "    command: echo hi\n",
        ),
        pattern = pattern,
        name_template = name_template.replace("{dir}", &dir_fwd),
    ));

    let manifest = manifest::from_str(&yaml).expect("parse");

    if expected_partial.is_empty() {
        assert!(manifest.targets.is_empty(), "{description}");
    } else {
        let prefix_fwd = format!("{dir_fwd}/");
        let prefix_back = format!("{dir_str}\\");
        let names: Vec<_> = target_names(&manifest)
            .into_iter()
            .map(|n| n.replace(&prefix_fwd, "").replace(&prefix_back, ""))
            .collect();
        assert_eq!(names, expected_partial, "{description}");
    }
}

#[rstest]
#[rstest]
fn glob_invalid_pattern_errors() {
    let yaml =
        manifest_yaml("targets:\n  - foreach: glob('[')\n    name: bad\n    command: echo hi\n");
    let err = manifest::from_str(&yaml).expect_err("invalid pattern should error");
    let msg = format!("{err:#}");
    assert!(msg.contains("invalid glob pattern"), "{msg}");
}

#[rstest]
fn glob_accepts_windows_path_separators(temp_dir: tempfile::TempDir) {
    fs::write(temp_dir.path().join("a.txt"), "a").expect("write a");
    fs::write(temp_dir.path().join("b.txt"), "b").expect("write b");
    let dir_fwd = temp_dir.path().display().to_string();
    let dir_win = dir_fwd.replace('/', "\\\\");
    let pattern = format!("{dir_win}\\\\*.txt");
    let yaml = manifest_yaml(&format!(
        concat!(
            "targets:\n",
            "  - foreach: glob('{pattern}')\n",
            "    name: \"{{{{ item }}}}\"\n",
            "    command: echo hi\n",
        ),
        pattern = pattern,
    ));
    let manifest = manifest::from_str(&yaml).expect("parse");
    let prefix_fwd = format!("{dir_fwd}/");
    let names: Vec<_> = target_names(&manifest)
        .into_iter()
        .map(|n| n.replace(&prefix_fwd, "").replace(".txt", ".out"))
        .collect();
    assert_eq!(names, vec!["a.out", "b.out"]);
}

#[cfg(windows)]
#[rstest]
fn glob_is_case_sensitive_on_windows(temp_dir: tempfile::TempDir) {
    fs::write(temp_dir.path().join("UPPER.TXT"), "x").expect("write file");
    let pattern = format!("{}/*.txt", temp_dir.path().display());
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
    assert!(manifest.targets.is_empty());
}
