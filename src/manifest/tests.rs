use super::*;
use rstest::{fixture, rstest};
use serde::de::Error as _;

#[test]
fn yaml_error_without_location_defaults_to_first_line() {
    let err = YamlError::custom("boom");
    let msg = map_yaml_error(err, "", "test").to_string();
    assert!(msg.contains("line 1, column 1"), "message: {msg}");
}

#[test]
fn yaml_hint_needles_are_lowercase() {
    for (needle, _) in YAML_HINTS {
        assert_eq!(
            needle,
            needle.to_lowercase(),
            "needle not lower-case: {needle}"
        );
    }
}

#[test]
fn glob_paths_invalid_pattern_sets_syntax_error() {
    let err = super::glob_paths("[").expect_err("expected pattern error");
    assert_eq!(err.kind(), minijinja::ErrorKind::SyntaxError);
}

#[cfg(unix)]
#[rstest]
#[case("\\[")]
#[case("\\]")]
#[case("\\{")]
#[case("\\}")]
fn normalize_separators_preserves_bracket_escape_variants(#[case] pat: &str) {
    assert_eq!(super::normalize_separators(pat), pat);
}

#[cfg(unix)]
#[rstest]
#[case("a\\b\\*", "a/b\\*")]
#[case("a\\b\\?", "a/b\\?")]
#[case("config\\*.yml", "config\\*.yml")]
#[case("data\\?x.csv", "data\\?x.csv")]
fn normalize_separators_preserves_wildcard_escape_variants(
    #[case] pat: &str,
    #[case] expected: &str,
) {
    assert_eq!(super::normalize_separators(pat), expected);
}

#[cfg(unix)]
#[rstest]
#[case("assets/\\*.\\?", "assets/\\*.\\?")]
#[case("src/\\[a\\].c", "src/\\[a\\].c")]
#[case("build/\\{debug,release\\}/lib", "build/\\{debug,release\\}/lib")]
fn normalize_separators_preserves_specific_escape_patterns(
    #[case] pat: &str,
    #[case] expected: &str,
) {
    // Note: a '\\' before '*' that is followed by '.' is treated as a separator,
    // hence the double '/' in the first case.
    assert_eq!(super::normalize_separators(pat), expected);
}

#[fixture]
fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().expect("temp dir")
}

#[cfg(unix)]
#[rstest]
#[case("\\*", "*")]
#[case("\\?", "?")]
#[case("prefix\\*suffix", "prefix*suffix")]
#[case("pre\\?post", "pre?post")]
#[case("mid\\*-", "mid*-")]
#[case("mid\\?_", "mid?_")]
fn glob_paths_treats_escaped_wildcards_as_literals(
    tmp: tempfile::TempDir,
    #[case] pattern_suffix: &str,
    #[case] filename: &str,
) {
    std::fs::write(tmp.path().join(filename), "a").expect("write file");
    std::fs::write(tmp.path().join("other"), "b").expect("write file");
    let pattern = format!("{}/{}", tmp.path().display(), pattern_suffix);
    let out = super::glob_paths(&pattern).expect("glob ok");
    assert_eq!(
        out,
        vec![format!("{}/{}", tmp.path().display(), filename).replace('\\', "/"),],
    );
}

#[cfg(unix)]
#[rstest]
#[case(
    vec![("sub", true), ("a.txt", false)], // (name, is_dir)
    "*",
    vec!["a.txt"],
    "ignores directories and matches only files"
)]
#[case(
    vec![
        ("dir", true),
        ("dir/prefix*suffix", true),
        ("dir/prefix*suffix/next", false),
    ],
    "dir/prefix\\*suffix/*",
    vec!["dir/prefix*suffix/next"],
    "treats escaped '*' as literal inside a segment across subsequent segments",
)]
#[case(
    vec![("[ab]", false), ("b", false)],
    "\\[ab\\]",
    vec!["[ab]"],
    "respects bracket escapes - treats as literals"
)]
#[case(
    vec![("{debug,release}", false)],
    "\\{debug,release\\}",
    vec!["{debug,release}"],
    "treats braces as literals when escaped"
)]
fn glob_paths_behavior_scenarios(
    tmp: tempfile::TempDir,
    #[case] files_to_create: Vec<(&str, bool)>, // (name, is_directory)
    #[case] pattern_suffix: &str,
    #[case] expected_matches: Vec<&str>,
    #[case] description: &str,
) {
    for (name, is_dir) in &files_to_create {
        let path = tmp.path().join(name);
        if *is_dir {
            std::fs::create_dir(&path).expect("create dir");
        } else {
            std::fs::write(&path, "content").expect("write file");
        }
    }

    let pattern = format!("{}/{}", tmp.path().display(), pattern_suffix);
    let mut result = super::glob_paths(&pattern).expect("glob ok");
    result.sort();
    let mut expected: Vec<String> = expected_matches
        .iter()
        .map(|name| format!("{}/{}", tmp.path().display(), name).replace('\\', "/"))
        .collect();
    expected.sort();
    assert_eq!(result, expected, "Test case: {description}");
}
