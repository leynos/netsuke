//! Tests for file globbing via the `glob()` Jinja helper.

use netsuke::{
    ast::{NetsukeManifest, StringOrList},
    manifest,
};
use rstest::{fixture, rstest};
use std::{fs, path::Path};
use test_support::display_error_chain;

#[derive(Debug)]
struct TestFiles<'a> {
    pub files: &'a [(&'a str, &'a str)],
    pub dirs: &'a [&'a str],
}

#[derive(Debug)]
struct GlobTestCase<'a> {
    pub setup: TestFiles<'a>,
    pub pattern_suffix: &'a str,
    pub name_template: &'a str,
    pub expected_partial: &'a [&'a str],
    pub description: &'a str,
}

#[derive(Debug)]
struct BraceErrorTestCase<'a> {
    pub pattern: &'a str,
    pub expected: &'a str,
}

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
#[case(GlobTestCase {
    setup: TestFiles { files: &[("b.txt", "b"), ("a.txt", "a")], dirs: &[] },
    pattern_suffix: "*.txt",
    name_template: "{{ item | replace('{dir}/', '') | replace('{dir}\\\\', '') | replace('.txt', '.out') }}",
    expected_partial: &["a.out", "b.out"],
    description: "expands and sorts matches",
})]
#[case(GlobTestCase {
    setup: TestFiles { files: &[], dirs: &[] },
    pattern_suffix: "*.nomatch",
    name_template: "none",
    expected_partial: &[],
    description: "no targets when pattern has no matches",
})]
#[case(GlobTestCase {
    setup: TestFiles { files: &[("sub/x.txt", "x")], dirs: &[] },
    pattern_suffix: "*.txt",
    name_template: "bad",
    expected_partial: &[],
    description: "wildcards must not cross '/'",
})]
#[case(GlobTestCase {
    setup: TestFiles { files: &[("sub/x.txt", "x")], dirs: &[] },
    pattern_suffix: "**/*.txt",
    name_template: "{{ item | replace('{dir}/', '') }}",
    expected_partial: &["sub/x.txt"],
    description: "** spans directories",
})]
#[case(GlobTestCase {
    setup: TestFiles { files: &[(".hidden.txt", "h")], dirs: &[] },
    pattern_suffix: "*.txt",
    name_template: "{{ item }}",
    expected_partial: &[".hidden.txt"],
    description: "dotfiles should match",
})]
#[case(GlobTestCase {
    setup: TestFiles { files: &[("UPPER.TXT", "x")], dirs: &[] },
    pattern_suffix: "*.txt",
    name_template: "fail",
    expected_partial: &[],
    description: "glob should be case-sensitive",
})]
#[case(GlobTestCase {
    setup: TestFiles { files: &[("a.txt", "a")], dirs: &["sub"] },
    pattern_suffix: "*",
    name_template: "{{ item }}",
    expected_partial: &["a.txt"],
    description: "should filter directories",
})]
fn test_glob_behavior(temp_dir: tempfile::TempDir, #[case] case: GlobTestCase) {
    create_test_files(temp_dir.path(), case.setup.files);
    create_test_dirs(temp_dir.path(), case.setup.dirs);

    let dir_str = temp_dir.path().display().to_string();
    let dir_fwd = dir_str.replace('\\', "/");
    let pattern = format!("{dir_str}/{}", case.pattern_suffix);
    let yaml = manifest_yaml(&format!(
        concat!(
            "targets:\n",
            "  - foreach: glob('{pattern}')\n",
            "    name: \"{name_template}\"\n",
            "    command: echo hi\n",
        ),
        pattern = pattern,
        name_template = case
            .name_template
            .replace("{dir}/", &format!("{dir_fwd}/"))
            .replace("{dir}\\\\", &format!("{dir_str}\\\\\\\\")),
    ));

    let manifest = manifest::from_str(&yaml).expect("parse");

    if case.expected_partial.is_empty() {
        assert!(manifest.targets.is_empty(), "{}", case.description);
    } else {
        let prefix_fwd = format!("{dir_fwd}/");
        let prefix_back = format!("{dir_str}\\");
        let names: Vec<_> = target_names(&manifest)
            .into_iter()
            .map(|n| n.replace(&prefix_fwd, "").replace(&prefix_back, ""))
            .collect();
        assert_eq!(names, case.expected_partial, "{}", case.description);
    }
}

#[test]
fn glob_unmatched_bracket_errors() {
    let yaml =
        manifest_yaml("targets:\n  - foreach: glob('[')\n    name: bad\n    command: echo hi\n");
    let err = manifest::from_str(&yaml).expect_err("invalid pattern should error");
    let msg = format!("{err:#}");
    assert!(msg.contains("invalid glob pattern"), "{msg}");
}

#[rstest]
#[case(BraceErrorTestCase { pattern: "{", expected: "unmatched '{'" })]
#[case(BraceErrorTestCase { pattern: "}", expected: "unmatched '}'" })]
#[case(BraceErrorTestCase { pattern: "foo{bar{baz.txt", expected: "unmatched '{'" })]
#[case(BraceErrorTestCase { pattern: "{a,b{c,d}", expected: "unmatched '{'" })]
fn glob_unmatched_brace_errors(#[case] case: BraceErrorTestCase) {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: glob('{pattern}')\n    name: bad\n    command: echo hi\n",
        pattern = case.pattern,
    ));
    let err = manifest::from_str(&yaml).expect_err("invalid pattern should error");
    let msg = display_error_chain(err.as_ref());
    assert!(msg.contains("invalid glob pattern"), "{msg}");
    assert!(msg.contains(case.expected), "{msg}");
}

#[cfg(unix)]
#[rstest]
#[case(BraceErrorTestCase { pattern: "\\\\{foo}", expected: "unmatched '}'" })]
#[case(BraceErrorTestCase { pattern: "foo\\\\{bar}", expected: "unmatched '}'" })]
#[case(BraceErrorTestCase { pattern: "{foo\\\\}", expected: "unmatched '{'" })]
fn glob_unmatched_brace_errors_with_escapes(#[case] case: BraceErrorTestCase) {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: glob('{pattern}')\n    name: bad\n    command: echo hi\n",
        pattern = case.pattern,
    ));
    let err = manifest::from_str(&yaml).expect_err("invalid pattern should error");
    let msg = display_error_chain(err.as_ref());
    assert!(msg.contains("invalid glob pattern"), "{msg}");
    assert!(msg.contains(case.expected), "{msg}");
}

#[test]
fn glob_unmatched_opening_brace_reports_position() {
    let yaml =
        manifest_yaml("targets:\n  - foreach: glob('{')\n    name: bad\n    command: echo hi\n");
    let err = manifest::from_str(&yaml).expect_err("invalid pattern should error");
    let msg = display_error_chain(err.as_ref());
    assert!(msg.contains("unmatched '{' at position 0"), "{msg}");
}

#[rstest]
#[case(BraceErrorTestCase { pattern: "\\\\{", expected: "" })]
#[case(BraceErrorTestCase { pattern: "\\\\}", expected: "" })]
#[case(BraceErrorTestCase { pattern: "foo\\\\}", expected: "" })]
#[case(BraceErrorTestCase { pattern: "foo{bar\\\\}baz}", expected: "" })]
#[case(BraceErrorTestCase { pattern: "foo\\\\{bar", expected: "" })]
fn glob_escaped_braces_are_literals(#[case] case: BraceErrorTestCase) {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: glob('{pattern}')\n    name: ok\n    command: echo hi\n",
        pattern = case.pattern,
    ));
    let manifest = manifest::from_str(&yaml).expect("escaped brace should parse");
    assert!(manifest.targets.is_empty());
}
#[cfg(windows)]
#[rstest]
#[case(BraceErrorTestCase { pattern: "\\{foo", expected: "unmatched '{'" })]
#[case(BraceErrorTestCase { pattern: "foo\\}", expected: "unmatched '}'" })]
fn glob_windows_backslash_does_not_escape_braces(#[case] case: BraceErrorTestCase) {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: glob('{pattern}')\n    name: bad\n    command: echo hi\n",
        pattern = case.pattern,
    ));
    let err = manifest::from_str(&yaml).expect_err("invalid pattern should error");
    let msg = display_error_chain(err.as_ref());
    assert!(msg.contains("invalid glob pattern"), "{msg}");
    assert!(msg.contains(case.expected), "{msg}");
}
#[rstest]
#[case(BraceErrorTestCase { pattern: "[{}]", expected: "" })] // braces as literals inside a class
#[case(BraceErrorTestCase { pattern: "[{]", expected: "" })] // unmatched '{' inside class must NOT error
#[case(BraceErrorTestCase { pattern: "x{a,{b,c}}.txt", expected: "" })] // nested braces
#[case(BraceErrorTestCase { pattern: "{a,{b,{c,{d,e}}}}", expected: "" })] // deeply nested braces
fn glob_braces_in_classes_and_nested(#[case] case: BraceErrorTestCase) {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: glob('{pattern}')\n    name: ok\n    command: echo hi\n",
        pattern = case.pattern,
    ));
    let _ = manifest::from_str(&yaml).expect("pattern should parse");
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
