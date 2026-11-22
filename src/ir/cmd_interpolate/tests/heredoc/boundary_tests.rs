//! Unit tests for the boundary at which a pending heredoc body begins.
//!
//! POSIX starts a queued body after the newline that ends the command line
//! declaring it, not after a newline inside a quoted argument. Starting the
//! body early consumes the closing quote as body text, so the scanner's quote
//! state stays stale and a later marker is encoded for quotes the shell has
//! already closed. A path carrying `;` then runs outside those quotes.
//!
//! A newline the traversal cannot place at a command-line boundary — one
//! inside `$()`, backticks, arithmetic, an open `${ }`, or a frame that has
//! already closed — is rejected rather than guessed at.

use super::super::*;
use super::support::{
    HOSTILE_UNQUOTED, Lowering, Recipe, assert_both_traversals_lower,
    assert_both_traversals_lower_with, assert_both_traversals_reject_heredoc_boundary,
    hostile_bindings,
};
use rstest::rstest;

/// Build a command whose multiline quoted argument sits before the body.
///
/// The argument spans one unquoted newline, which is not the command-line
/// boundary that starts a heredoc body. The marker after the terminator is the
/// discriminator: it must use the encoding an unquoted site requires.
fn quoted_argument_recipe(quote: char) -> (String, String) {
    let template =
        format!("cat <<EOF {quote}argument\ncontinues{quote}\n{INS_TOKEN}\nEOF\necho {OUTS_TOKEN}");
    let expected = format!(
        "cat <<EOF {quote}argument\ncontinues{quote}\n{INS_TOKEN}\nEOF\necho {HOSTILE_UNQUOTED}"
    );
    (template, expected)
}

// Verify a newline inside a quoted argument does not start the pending body.
//
// The shell keeps reading the command line through the quoted newline and
// begins the heredoc body only after the closing quote's line. Starting it at
// the first newline swallows the closing quote, leaves the scanner inside a
// quote the shell has closed, and encodes the post-terminator marker with
// double-quoted escaping. The hostile path's semicolon is then bare in
// executable text.
#[rstest]
#[case::double_quoted('"')]
#[case::single_quoted('\'')]
fn quoted_newline_defers_the_pending_body(#[case] quote: char) {
    let (template, expected) = quoted_argument_recipe(quote);
    assert_both_traversals_lower_with(
        Recipe(&template),
        &hostile_bindings(),
        &Lowering {
            text: &expected,
            rule: "a newline inside a quoted argument must not start the pending heredoc body",
        },
    );
}

// Verify an apparent delimiter inside an open quote does not start the body.
//
// The line `EOF` is quoted argument text, not a terminator, because the body
// has not begun. A scanner that starts the body at the quoted newline ends it
// at that fake line, then lowers the body marker as executable text. The true
// body must stay byte-for-byte inert and the marker after the real terminator
// must still use the unquoted encoding.
#[test]
fn an_apparent_delimiter_inside_a_quoted_argument_does_not_start_the_body() {
    let template =
        format!("cat <<EOF \"argument\nEOF\ncontinues\"\n{INS_TOKEN}\nEOF\necho {OUTS_TOKEN}");
    let expected = format!(
        "cat <<EOF \"argument\nEOF\ncontinues\"\n{INS_TOKEN}\nEOF\necho {HOSTILE_UNQUOTED}"
    );
    assert_both_traversals_lower_with(
        Recipe(&template),
        &hostile_bindings(),
        &Lowering {
            text: &expected,
            rule: "a fake delimiter inside an open quote must not end a body that has not begun",
        },
    );
}

// Verify the newline that ends a declaration comment still starts the body.
//
// A comment runs to the end of its command line, so its newline is the
// command-line boundary. Routing comment newlines through the same decision as
// ordinary text must keep this supported shape working.
#[test]
fn a_comment_newline_still_starts_the_pending_body() {
    let template = format!("cat <<EOF # declaration\n{INS_TOKEN}\nEOF\necho {OUTS_TOKEN}");
    let expected = format!("cat <<EOF # declaration\n{INS_TOKEN}\nEOF\necho {HOSTILE_UNQUOTED}");
    assert_both_traversals_lower_with(
        Recipe(&template),
        &hostile_bindings(),
        &Lowering {
            text: &expected,
            rule: "a comment newline ends the declaring command line and starts the body",
        },
    );
}

// Verify a complete heredoc inside one substitution frame remains supported.
//
// A `$()` body is its own command line, so a declaration there starts its body
// at that frame's newline. The frame identifier must match the declaration's
// owner for the body to begin.
#[test]
fn a_complete_heredoc_inside_one_substitution_frame_is_accepted() {
    let template = format!("x=$(cat <<EOF\n{INS_TOKEN}\nEOF\n)\necho {OUTS_TOKEN}");
    let expected = format!("x=$(cat <<EOF\n{INS_TOKEN}\nEOF\n)\necho output");
    assert_both_traversals_lower(
        Recipe(&template),
        &Lowering {
            text: &expected,
            rule: "a heredoc wholly inside one substitution frame must stay trackable",
        },
    );
}

// Verify a pending declaration at a newline the scanner cannot place is rejected.
//
// Each case has a declaration waiting when the newline falls inside a region
// whose end the tracker cannot correlate with the declaring command line.
// Activating the body there would move the terminator search into text the
// shell still treats as part of an expansion, so the traversal must fail
// closed. `closed_frame` covers a declaration whose `$()` frame ended before
// the newline: the owner no longer matches the region the traversal is in.
#[rstest]
#[case::command_substitution("cat <<EOF $(echo hi\n)\nbody\nEOF")]
#[case::backticks("cat <<EOF `echo hi\n`\nbody\nEOF")]
#[case::arithmetic_expansion("cat <<EOF $((1\n+1))\nbody\nEOF")]
#[case::parameter_expansion("cat <<EOF ${x\n}\nbody\nEOF")]
#[case::closed_frame("cat $(echo <<EOF)\nbody\nEOF")]
fn pending_body_at_an_untracked_newline_is_rejected(#[case] template: &str) {
    assert_both_traversals_reject_heredoc_boundary(
        Recipe(template),
        "a newline that is not the declaring command line's boundary must fail closed",
    );
}

// Verify nested-region newlines without a pending declaration stay supported.
//
// The rejection is about placing a queued body, not about the regions
// themselves. These recipes exercise the same newlines with no declaration
// waiting, so the marker after each one must still be lowered normally.
#[rstest]
#[case::command_substitution(format!("echo $(printf '%s' hi\n)\necho {OUTS_TOKEN}"))]
#[case::backticks(format!("echo `printf '%s' hi\n`\necho {OUTS_TOKEN}"))]
#[case::arithmetic_expansion(format!("echo $((1 +\n2))\necho {OUTS_TOKEN}"))]
#[case::parameter_expansion(format!("echo ${{x\n:-default}}\necho {OUTS_TOKEN}"))]
fn nested_region_newlines_without_a_pending_declaration_are_accepted(#[case] template: String) {
    let expected = template.replace(OUTS_TOKEN, "output");
    assert_both_traversals_lower(
        Recipe(&template),
        &Lowering {
            text: &expected,
            rule: "a region newline with no queued body must leave ordinary lowering intact",
        },
    );
}

// Verify the boundary rule is shared by POSIX and Bash script traversals.
//
// Both script shells use the same scanner, and the shell reading the finished
// script applies the same heredoc rule, so neither may accept a boundary the
// other rejects.
#[rstest]
#[case::posix(RecipeShell::Posix)]
#[case::bash(RecipeShell::Bash)]
fn untracked_newlines_fail_closed_for_each_posix_script_shell(#[case] shell: RecipeShell) {
    let bindings = CommandBindings::new(&[], &[], shell);
    let template = "cat <<EOF $(echo hi\n)\nbody\nEOF";
    for result in [
        substitute(template, &bindings),
        substitute_script(template, &bindings),
    ] {
        let error = result.expect_err("a pending body at a nested-region newline must fail closed");
        assert!(
            matches!(error, IrGenError::InvalidCommand { .. }),
            "expected an invalid-command error, got {error:?}"
        );
        assert!(
            error.to_string().contains("unsupported heredoc boundary"),
            "the diagnostic must name the rejected boundary: {error}"
        );
    }
}

// Verify an escape-bearing ANSI-C quote is rejected wherever it appears.
//
// Bash reads `$'a\'b'` as one ANSI-C string, so the newline after it ends the
// command line and starts a body declared on that line. POSIX single-quote
// rules instead end the string at the `\'`, and the tracker then reopens it at
// the real closer: a `<<EOF` between the two is never recognised, so the body
// marker is lowered with a quoted encoding and Bash expands it. Rejecting only
// the after-declaration order leaves that earlier order exploitable, so the
// detector is not gated on the pending queue.
#[rstest]
#[case::after_declaration("cat <<EOF $'a\\'b'\nbody\nEOF")]
#[case::before_declaration("cat $'a\\'b' <<EOF\nbody\nEOF")]
#[case::split_by_continuation("cat $\\\n'a\\'b' <<EOF\nbody\nEOF")]
#[case::before_declaration_with_marker(&format!("cat $'a\\'b' <<EOF\n{INS_TOKEN}\nEOF\necho {OUTS_TOKEN}"))]
fn ambiguous_ansic_quote_is_rejected(#[case] template: &str) {
    let bindings = CommandBindings::new(&[], &[], RecipeShell::Bash);
    for result in [
        substitute(template, &bindings),
        substitute_script(template, &bindings),
    ] {
        let error = result.expect_err("an escape-bearing ANSI-C quote must fail closed");
        assert!(
            matches!(error, IrGenError::InvalidCommand { .. }),
            "expected an invalid-command error, got {error:?}"
        );
        assert!(
            error.to_string().contains("escape-bearing ANSI-C quote"),
            "the diagnostic must name the untrackable quote: {error}"
        );
    }
}

// Verify an escape-free ANSI-C quote keeps the supported lowering.
//
// Without a backslash the ANSI-C and POSIX readings end at the same quote, so
// no later decision changes and the recipe lowers as before.
#[rstest]
#[case::boundary("cat <<EOF $'ab'\nbody\nEOF")]
#[case::with_marker(&format!("cat <<EOF $'ab'\nbody {INS_TOKEN}\nEOF\necho {OUTS_TOKEN}"))]
#[case::plain("echo $'plain text'")]
fn escape_free_ansic_quote_still_lowers(#[case] template: &str) {
    let expected = template.replace(OUTS_TOKEN, "output");
    assert_both_traversals_lower_with(
        Recipe(template),
        &CommandBindings::new(
            &[Utf8PathBuf::from("input")],
            &[Utf8PathBuf::from("output")],
            RecipeShell::Posix,
        ),
        &Lowering {
            text: &expected,
            rule: "an escape-free ANSI-C quote agrees with the POSIX reading",
        },
    );
}

// Verify a heredoc body may contain an ANSI-C string without being scanned.
//
// Body text is copied before the ANSI-C detector runs, so Netsuke neither
// rewrites nor rejects the string. The shell still reads the body: an
// unquoted delimiter expands command substitutions there, and the copied text
// is what it expands. Rejecting the string would refuse an input the shell
// accepts, so the detector applies only to scanned command text.
#[rstest]
#[case::body_command_substitution("cat <<EOF\n$(printf '%s' $'a\\'b')\nEOF")]
fn ansic_quote_inside_a_heredoc_body_is_copied(#[case] template: &str) {
    let bindings = CommandBindings::new(&[], &[], RecipeShell::Bash);
    let lowered = substitute_script(template, &bindings)
        .expect("heredoc body text must be copied without the ANSI-C check");
    assert_eq!(lowered, template, "a heredoc body must be copied verbatim");
}
