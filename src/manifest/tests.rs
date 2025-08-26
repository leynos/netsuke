use super::*;
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
#[test]
fn normalise_separators_preserves_escapes() {
    assert_eq!(super::normalise_separators("\\["), "\\[");
    assert_eq!(super::normalise_separators("\\*"), "\\*");
    assert_eq!(super::normalise_separators("\\?"), "\\?");
}

#[test]
fn glob_paths_ignores_directories() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir(dir.path().join("sub")).expect("create dir");
    std::fs::write(dir.path().join("a.txt"), "a").expect("write file");
    let pattern = format!("{}/{}", dir.path().display(), "*");
    let out = super::glob_paths(&pattern).expect("glob ok");
    assert_eq!(
        out,
        vec![format!("{}/a.txt", dir.path().display()).replace('\\', "/")]
    );
}
