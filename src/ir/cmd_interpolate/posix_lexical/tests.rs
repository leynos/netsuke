//! Unit tests for POSIX heredoc delimiter interpolation and comment boundaries.

use super::super::{
    CommandBindings, INS_TOKEN, OUTS_TOKEN, RecipeShell, interpolate_command_with_bindings,
    interpolate_script_with_bindings,
};

use camino::Utf8PathBuf;
use rstest::rstest;

/// Path whose unquoted encoding is executable shell syntax inside double quotes.
const HOSTILE_PATH: &str = "x\"; touch injected; echo \"y";

/// Expect the hostile path escaped for insertion between existing double quotes.
const HOSTILE_DOUBLE_QUOTED: &str = "x\\\"; touch injected; echo \\\"y";

/// Build POSIX bindings that expose a wrong quote-context encoding.
fn hostile_posix_bindings() -> CommandBindings {
    CommandBindings::new(
        &[Utf8PathBuf::from(HOSTILE_PATH)],
        &[Utf8PathBuf::from(HOSTILE_PATH)],
        RecipeShell::Posix,
    )
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

/// Verify a non-separator before `#` cannot hide a later quote transition.
///
/// POSIX shells end a word only at blank, newline, or an operator. A no-break
/// space is word data, so the quote after `#` stays executable and must open a
/// double-quoted region for the marker on the next line. Treating it as
/// whitespace would skip that transition and encode the path as unquoted text,
/// which the real double-quoted region would execute.
#[rstest]
#[case::no_break_space('\u{00a0}')]
#[case::ideographic_space('\u{3000}')]
#[case::em_space('\u{2003}')]
#[case::next_line('\u{0085}')]
#[case::carriage_return('\r')]
#[case::vertical_tab('\u{000b}')]
#[case::form_feed('\u{000c}')]
#[case::non_ascii_word_letter('\u{00e9}')]
fn non_ascii_whitespace_before_hash_preserves_executable_quote_context(#[case] character: char) {
    let bindings = hostile_posix_bindings();
    let template = format!("echo x{character}#\"\necho {INS_TOKEN}\"");
    let expected = format!("echo x{character}#\"\necho {HOSTILE_DOUBLE_QUOTED}\"");

    assert_eq!(
        interpolate_command_with_bindings(&template, &bindings)
            .expect("a non-separator before '#' must keep the later quote executable"),
        expected
    );
    assert_eq!(
        interpolate_script_with_bindings(&template, &bindings)
            .expect("a non-separator before '#' must keep the later quote executable"),
        expected
    );
}

/// Verify each POSIX separator lets `#` start an inert comment.
///
/// The comment carries a marker and an unmatched quote; a scanner that ends
/// the comment early would treat either as executable syntax and mis-encode
/// the double-quoted marker on the following line.
#[rstest]
#[case::space("echo x ")]
#[case::tab("echo x\t")]
#[case::newline("echo x\n")]
#[case::beginning_of_input("")]
#[case::semicolon("echo x;")]
#[case::ampersand("echo x&")]
#[case::pipe("echo x|")]
#[case::subshell_open("echo x(")]
#[case::subshell_close("echo x)")]
#[case::input_redirect("echo x<")]
#[case::output_redirect("echo x>")]
fn posix_separators_before_hash_start_inert_comments(#[case] prefix: &str) {
    let bindings = hostile_posix_bindings();
    let template = format!("{prefix}# {INS_TOKEN} unmatched '\necho \"{OUTS_TOKEN}\"");
    let expected = format!("{prefix}# {INS_TOKEN} unmatched '\necho \"{HOSTILE_DOUBLE_QUOTED}\"");

    assert_eq!(
        interpolate_script_with_bindings(&template, &bindings)
            .expect("a comment after a separator must stay inert"),
        expected
    );
}

/// Verify a non-ASCII character keeps a heredoc delimiter word intact.
///
/// The delimiter is `EOF` followed by a no-break space and `X`, so the body
/// line `EOF` does not terminate it and the marker inside the body stays
/// literal. A scanner that ended the delimiter at the no-break space would
/// close the body early and interpolate that marker as executable text.
#[test]
fn non_ascii_delimiter_character_keeps_heredoc_body_inert() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("$(touch injected)")],
        &[Utf8PathBuf::from("output")],
        RecipeShell::Posix,
    );
    let template = format!("cat <<EOF\u{00a0}X\nEOF\n{INS_TOKEN}\nEOF\u{00a0}X\n");

    assert_eq!(
        interpolate_script_with_bindings(&template, &bindings)
            .expect("a heredoc delimiter containing word data must keep its body inert"),
        template
    );
}
