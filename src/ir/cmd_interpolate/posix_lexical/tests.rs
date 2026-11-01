//! Unit tests for POSIX heredoc delimiter interpolation.

use super::super::{
    CommandBindings, INS_TOKEN, IrGenError, OUTS_TOKEN, RecipeShell,
    interpolate_command_with_bindings, interpolate_script_with_bindings,
};

use camino::Utf8PathBuf;
use rstest::rstest;

/// Build bindings for one simple input and output path pair.
fn bindings() -> CommandBindings {
    CommandBindings::new(
        &[Utf8PathBuf::from("input")],
        &[Utf8PathBuf::from("output")],
        RecipeShell::Posix,
    )
}

/// Reject a delimiter marker preceded by an active escape in either traversal.
///
/// A single backslash escapes nothing once the marker is replaced, because the
/// replacement's first character decides what the backslash modifies. Both an
/// unquoted and a double-quoted delimiter word must fail closed.
#[rstest]
#[case::unquoted(format!("cat <<\\{OUTS_TOKEN}\nbody\n\\output"))]
#[case::double_quoted(format!("cat <<\"\\{OUTS_TOKEN}\"\nbody\n\\output"))]
fn delimiter_markers_after_an_active_escape_are_rejected(#[case] template: String) {
    let bindings = bindings();
    for lowering in [
        interpolate_script_with_bindings
            as fn(&str, &CommandBindings) -> Result<String, IrGenError>,
        interpolate_command_with_bindings,
    ] {
        let error = lowering(&template, &bindings)
            .expect_err("a delimiter marker after an active escape must fail closed");
        assert!(
            matches!(
                error,
                IrGenError::InvalidCommand { ref command, .. } if command == &template
            ),
            "unexpected error: {error:?}"
        );
    }
}

/// Keep accepting escaped delimiter backslashes that form a pair.
///
/// The pair is copied into the rendered delimiter, the body stays inert, and
/// interpolation resumes once the terminator line ends it.
#[rstest]
#[case::paired_backslashes(
    format!("cat <<\\\\{OUTS_TOKEN}\nbody ' \\ {INS_TOKEN}\n\\output\necho {INS_TOKEN}"),
    format!("cat <<\\\\output\nbody ' \\ {INS_TOKEN}\n\\output\necho input")
)]
#[case::single_quoted(
    format!("cat <<'\\{OUTS_TOKEN}'\nbody ' \\ {INS_TOKEN}\n\\output\necho {INS_TOKEN}"),
    format!("cat <<'\\output'\nbody ' \\ {INS_TOKEN}\n\\output\necho input")
)]
fn escaped_delimiter_backslashes_keep_markers_eligible(
    #[case] template: String,
    #[case] expected: String,
) {
    assert_eq!(
        interpolate_script_with_bindings(&template, &bindings())
            .expect("an escaped delimiter backslash must keep the marker eligible"),
        expected
    );
}

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
