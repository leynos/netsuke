//! Unit tests for heredoc delimiter words and their interpolation context.
//!
//! A delimiter ends only at a POSIX word boundary, so non-ASCII whitespace is
//! word data rather than a separator. These cases pin that boundary rule and
//! the quote-removal and rejection contracts the tracker builds on.

use super::super::super::{
    CommandBindings, INS_TOKEN, OUTS_TOKEN, RecipeShell, interpolate_command_with_bindings,
    interpolate_script_with_bindings,
};
use super::super::{DelimiterRejection, delimiter::parse_delimiter};

use camino::Utf8PathBuf;
use rstest::rstest;

/// Verify Unicode whitespace stays inside an unquoted heredoc delimiter.
///
/// POSIX shells end a word only at a blank, newline, or operator. A Unicode
/// separator such as NBSP is therefore part of the delimiter, and the body
/// must stay inert until the full `EOF<separator>suffix` line appears. A
/// scanner that split the delimiter at that character would end the body early
/// and interpolate the marker inside it as executable text.
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

/// Verify a non-blank character after `<<` begins the delimiter word.
#[test]
fn unicode_character_after_the_heredoc_operator_belongs_to_the_delimiter() {
    let chars = ['<', '<', '\u{00A0}', 'E', 'O', 'F', '\n'];
    let parsed = parse_delimiter(&chars, 2).expect("a non-blank character begins a word");
    assert_eq!((parsed.text.as_str(), parsed.end), ("\u{00A0}EOF", 6));
}

/// Verify delimiter syntax the tracker cannot model is rejected.
#[rstest]
#[case::end_of_input(&[], 0, DelimiterRejection::MissingDelimiter)]
#[case::newline(&['\n'], 0, DelimiterRejection::MissingDelimiter)]
#[case::operator(&[';'], 0, DelimiterRejection::MissingDelimiter)]
#[case::leading_comment(&['#', 'E', 'O', 'F'], 0, DelimiterRejection::MissingDelimiter)]
#[case::unclosed_single_quote(&['\'', 'E'], 0, DelimiterRejection::UnclosedQuote)]
#[case::unclosed_double_quote(&['"', 'E'], 0, DelimiterRejection::UnclosedQuote)]
#[case::trailing_backslash(&['\\'], 0, DelimiterRejection::TrailingBackslash)]
#[case::line_continuation(&['\\', '\n'], 0, DelimiterRejection::LineContinuation)]
#[case::double_quoted_line_continuation(
    &['"', 'E', '\\', '\n'],
    0,
    DelimiterRejection::LineContinuation
)]
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

/// Verify supported delimiter quoting keeps its quote-removed text.
#[rstest]
#[case::plain("EOF", "EOF")]
#[case::single_quoted("'EOF'", "EOF")]
#[case::double_quoted("\"EOF\"", "EOF")]
#[case::mixed_quotes("E\"O\"F", "EOF")]
#[case::escaped("\\EOF", "EOF")]
#[case::escaped_dollar("\\$X", "$X")]
#[case::double_quoted_escaped_dollar("\"\\$X\"", "$X")]
#[case::escaped_backtick("\\`X", "`X")]
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

/// Verify a delimiter records whether quoting suppresses body expansion.
///
/// The recorded flag selects the body matching rule: a quoted delimiter keeps
/// physical lines, while an unquoted one joins backslash-continued lines first.
#[rstest]
#[case::plain("EOF", false)]
#[case::single_quoted("'EOF'", true)]
#[case::double_quoted("\"EOF\"", true)]
#[case::mixed_quotes("E\"O\"F", true)]
#[case::escaped("\\EOF", true)]
#[case::escaped_dollar("\\$X", true)]
#[case::escaped_backtick("\\`X", true)]
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
