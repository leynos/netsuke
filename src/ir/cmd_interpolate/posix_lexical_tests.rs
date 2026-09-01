//! Unit tests for POSIX heredoc delimiter interpolation.

use super::*;

use camino::Utf8PathBuf;

/// Verify escaped quotes do not change the marker context in a heredoc delimiter.
#[test]
fn escaped_heredoc_delimiter_quotes_preserve_marker_context() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("input")],
        &[Utf8PathBuf::from("x$y")],
        RecipeShell::Posix,
    );
    let template = format!("cat <<\\\"{OUTS_TOKEN}\nbody\n\"x$y");
    assert_eq!(
        interpolate_script_with_bindings(&template, &bindings)
            .expect("escaped delimiter quote must not select double-quoted encoding"),
        "cat <<\\\"x'$y'\nbody\n\"x$y"
    );
}

/// Verify heredoc declarations preserve each queued body before later lowering.
#[test]
fn multiple_heredoc_bodies_preserve_marker_literals() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("input path")],
        &[Utf8PathBuf::from("output path")],
        RecipeShell::Posix,
    );
    let template = format!(
        "cat <<FIRST <<'SECOND'\nfirst ' {INS_TOKEN}\nFIRST\nsecond \" {OUTS_TOKEN}\nSECOND\necho \"{INS_TOKEN}\""
    );
    let expected = format!(
        "cat <<FIRST <<'SECOND'\nfirst ' {INS_TOKEN}\nFIRST\nsecond \" {OUTS_TOKEN}\nSECOND\necho \"input path\""
    );
    assert_eq!(
        interpolate_script_with_bindings(&template, &bindings)
            .expect("each heredoc body must remain literal"),
        expected
    );
}
