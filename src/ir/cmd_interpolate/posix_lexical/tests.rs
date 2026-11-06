//! Unit tests for POSIX heredoc delimiter interpolation.

use super::super::{
    CommandBindings, INS_TOKEN, IrGenError, OUTS_TOKEN, RecipeShell,
    interpolate_command_with_bindings, interpolate_script_with_bindings,
};
use super::{DelimiterRejection, delimiter::parse_delimiter};

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

// Verify Unicode whitespace stays inside an unquoted heredoc delimiter.
//
// POSIX shells end a word only at a blank, newline, or operator. A Unicode
// separator such as NBSP is therefore part of the delimiter, and the body
// must stay inert until the full `EOF<separator>suffix` line appears.
#[rstest]
#[case::no_break_space('\u{00A0}')]
#[case::em_space('\u{2003}')]
#[case::vertical_tab('\u{000B}')]
#[case::form_feed('\u{000C}')]
#[case::carriage_return('\r')]
fn unicode_whitespace_stays_inside_a_heredoc_delimiter(#[case] separator: char) {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("input")],
        &[Utf8PathBuf::from("output")],
        RecipeShell::Posix,
    );
    let terminator = format!("EOF{separator}suffix");
    let template =
        format!("cat <<{terminator}\nbody\nEOF\n{INS_TOKEN}\n{terminator}\necho {OUTS_TOKEN}");
    let expected = format!("cat <<{terminator}\nbody\nEOF\n{INS_TOKEN}\n{terminator}\necho output");
    for lowered in [
        interpolate_command_with_bindings(&template, &bindings),
        interpolate_script_with_bindings(&template, &bindings),
    ] {
        assert_eq!(
            lowered.expect("a Unicode separator must remain inside the delimiter"),
            expected,
            "both traversals must keep the separator inside the delimiter"
        );
    }
}

// Verify a non-blank character after `<<` begins the delimiter word.
#[test]
fn unicode_character_after_the_heredoc_operator_belongs_to_the_delimiter() {
    let chars = ['<', '<', '\u{00A0}', 'E', 'O', 'F', '\n'];
    let parsed = parse_delimiter(&chars, 2).expect("a non-blank character begins a word");
    assert_eq!((parsed.text.as_str(), parsed.end), ("\u{00A0}EOF", 6));
}

// Verify delimiter syntax the tracker cannot model is rejected.
#[rstest]
#[case::end_of_input(&[], 0, DelimiterRejection::MissingDelimiter)]
#[case::newline(&['\n'], 0, DelimiterRejection::MissingDelimiter)]
#[case::operator(&[';'], 0, DelimiterRejection::MissingDelimiter)]
#[case::leading_comment(&['#', 'E', 'O', 'F'], 0, DelimiterRejection::MissingDelimiter)]
#[case::unclosed_single_quote(&['\'', 'E'], 0, DelimiterRejection::UnclosedQuote)]
#[case::unclosed_double_quote(&['"', 'E'], 0, DelimiterRejection::UnclosedQuote)]
#[case::trailing_backslash(&['\\'], 0, DelimiterRejection::TrailingBackslash)]
#[case::line_continuation(&['\\', '\n'], 0, DelimiterRejection::LineContinuation)]
#[case::unquoted_dollar(&['$', 'X'], 0, DelimiterRejection::UnsupportedExpansion)]
#[case::double_quoted_dollar(
    &['"', '$', 'X', '"'],
    0,
    DelimiterRejection::UnsupportedExpansion
)]
#[case::unquoted_backtick(&['`'], 0, DelimiterRejection::UnsupportedCommandSubstitution)]
#[case::double_quoted_backtick(
    &['"', '`', '"'],
    0,
    DelimiterRejection::UnsupportedCommandSubstitution
)]
fn unsupported_heredoc_delimiter_forms_are_rejected(
    #[case] chars: &[char],
    #[case] start: usize,
    #[case] expected: DelimiterRejection,
) {
    assert_eq!(parse_delimiter(chars, start).err(), Some(expected));
}

// Verify supported delimiter quoting keeps its quote-removed text.
#[rstest]
#[case::plain("EOF", "EOF")]
#[case::single_quoted("'EOF'", "EOF")]
#[case::double_quoted("\"EOF\"", "EOF")]
#[case::mixed_quotes("E\"O\"F", "EOF")]
#[case::escaped("\\EOF", "EOF")]
#[case::empty("''", "")]
#[case::literal_hash("E#F", "E#F")]
#[case::quoted_hash("'#'", "#")]
#[case::unicode("\u{00A0}EOF", "\u{00A0}EOF")]
fn supported_heredoc_delimiters_keep_their_quote_removed_text(
    #[case] delimiter: &str,
    #[case] expected: &str,
) {
    let chars: Vec<char> = delimiter.chars().collect();
    let parsed = parse_delimiter(&chars, 0).expect("a supported delimiter must parse as one word");
    assert_eq!((parsed.text.as_str(), parsed.end), (expected, chars.len()));
}

// Verify a delimiter records whether quoting suppresses body expansion.
//
// The recorded flag selects the body matching rule: a quoted delimiter keeps
// physical lines, while an unquoted one joins backslash-continued lines first.
#[rstest]
#[case::plain("EOF", false)]
#[case::single_quoted("'EOF'", true)]
#[case::double_quoted("\"EOF\"", true)]
#[case::mixed_quotes("E\"O\"F", true)]
#[case::escaped("\\EOF", true)]
#[case::literal_hash("E#F", false)]
#[case::unicode("\u{00A0}EOF", false)]
fn heredoc_delimiters_record_whether_they_are_quoted(
    #[case] delimiter: &str,
    #[case] expected: bool,
) {
    let chars: Vec<char> = delimiter.chars().collect();
    let parsed = parse_delimiter(&chars, 0).expect("a supported delimiter must parse as one word");
    assert_eq!(parsed.is_quoted, expected);
}
