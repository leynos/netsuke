//! POSIX heredoc tests for number signs that are not comments.
//!
//! A `#` can follow a blank without beginning a comment: `${name# #}` uses one
//! for pattern removal and `$(( ))` evaluates in a region where comment copying
//! would swallow a closing delimiter. Reading either as a comment copies the
//! rest of the line verbatim, so a heredoc declared on it is never queued and
//! marker lowering resumes in body lines the shell still expands.
//!
//! The `${ }` count is also scoped per `$( )` region. A brace inside a command
//! substitution belongs to that substitution, so it must not close an
//! expansion the enclosing region still holds open.

use super::super::*;
use super::support::{
    Lowering, Recipe, assert_both_traversals_lower, assert_declaration_keeps_its_body,
};

// Verify a number sign inside a parameter expansion cannot hide a heredoc.
//
// The declaration sits after the expansion on the same line, which is exactly
// what a wrongly started comment swallows. With the declaration swallowed the
// heredoc is never queued, so the body line would receive the input path: the
// injection this tracker exists to prevent. The expected text keeps the token
// in the body, so the case cannot pass by recognizing the declaration without
// queueing the body.
#[test]
fn a_number_sign_inside_a_parameter_expansion_does_not_hide_a_heredoc() {
    assert_declaration_keeps_its_body(
        &format!("cat ${{name# #}}<<EOF\n{INS_TOKEN}\nEOF\necho {OUTS_TOKEN}"),
        "a number sign inside a parameter expansion is not a comment",
    );
}

// Assert that a comment after `expansion` still hides its marker.
//
// This is the control for each case where a `#` inside the expansion is not a
// comment: a tracker that never left its expansion state would lower the
// marker here, so the retained token proves the tracking ends at the closing
// delimiter rather than at the line.
fn assert_comment_resumes_after(expansion: &str, rule: &str) {
    let line = format!("echo {expansion} # {OUTS_TOKEN}");
    assert_both_traversals_lower(Recipe(&line), &Lowering { text: &line, rule });
}

// Verify comment detection resumes once the parameter expansion closes.
#[test]
fn comment_detection_resumes_after_a_parameter_expansion() {
    assert_comment_resumes_after(
        "${name# #}",
        "a comment after a parameter expansion still hides its marker",
    );
}

// Assert that `substitution` keeps the enclosing `${ }` open, for either
// spelling of a command substitution.
//
// The shell keeps `${` open across the substitution, so both number signs are
// literal expansion text and the heredoc is still declared. A shared count —
// or comment copying that runs past the substitution's own delimiter — would
// read the following `#` as a comment and lose the declaration: the same
// injection reached by one more step. Only the spelling differs between
// `$( )` and backticks, so the shape is stated once here.
fn assert_substitution_keeps_the_declaration(substitution: &str, rule: &str) {
    assert_declaration_keeps_its_body(
        &format!(": ${{name:-{substitution} # #}} <<EOF\n{INS_TOKEN}\nEOF\necho {OUTS_TOKEN}"),
        rule,
    );
}

// Verify a brace inside `$( )` does not close the expansion that encloses it.
#[test]
fn a_brace_inside_a_command_substitution_does_not_close_a_parameter_expansion() {
    assert_substitution_keeps_the_declaration(
        "$(printf })",
        "a brace inside a command substitution belongs to it, not to the enclosing expansion",
    );
}

// Verify a number sign inside `$( )` is still a comment when the enclosing
// region is inside a parameter expansion.
//
// This is the other direction of the same scoping rule. The substitution opens
// its own count, so the `#` is recognized as a comment, `<<FAKE` is never
// queued as a declaration, and the marker after the substitution is lowered.
// A shared count would refuse the comment, queue `FAKE`, and let that body
// swallow the closing lines.
#[test]
fn a_comment_inside_a_command_substitution_is_recognized() {
    let template = format!(": ${{name:-$(printf x # <<FAKE\n)}}\necho {OUTS_TOKEN}");
    let expected = ": ${name:-$(printf x # <<FAKE\n)}\necho output";
    assert_both_traversals_lower(
        Recipe(&template),
        &Lowering {
            text: expected,
            rule: "a comment inside a command substitution still starts a comment",
        },
    );
}

// Verify a brace inside backticks does not close an enclosing `${ }`.
//
// Bash parses a backtick body as its own command region, so a brace there
// belongs to that region: `: ${name:-`printf }` # #} <<EOF` still declares the
// heredoc. A count shared across the backtick boundary reaches zero at that
// brace, reads the following number sign as a comment, loses the declaration,
// and lowers the body marker in a heredoc the shell expands.
#[test]
fn a_brace_inside_backticks_does_not_close_a_parameter_expansion() {
    assert_substitution_keeps_the_declaration(
        "`printf }`",
        "a brace inside backticks belongs to the backtick region",
    );
}

// Verify a number sign inside an arithmetic expansion is not a comment.
//
// Bash's arithmetic lexer reads a blank-led `#` as a comment that runs to the
// end of the line, so a shell rejects this fixture at run time with a bad
// substitution. The case pins Netsuke's own contract instead: comment copying
// never runs inside `$(( ))`, so no heredoc text is invented and the marker
// after the closing `))` is lowered. Without the arithmetic guard the number
// sign starts a comment and the marker is swallowed with the rest of the line.
#[test]
fn a_number_sign_inside_an_arithmetic_expansion_is_not_a_comment() {
    let template = format!("echo $((1 # x)) {OUTS_TOKEN}");
    assert_both_traversals_lower(
        Recipe(&template),
        &Lowering {
            text: "echo $((1 # x)) output",
            rule: "a number sign inside an arithmetic expansion is not a comment",
        },
    );
}

// Verify comment detection resumes once the arithmetic expansion closes.
#[test]
fn comment_detection_resumes_after_an_arithmetic_expansion() {
    assert_comment_resumes_after(
        "$((1 + 1))",
        "a comment after an arithmetic expansion still hides its marker",
    );
}

// Verify a `${` split by a backslash-newline still opens the expansion.
//
// The shell removes the continuation before it lexes the expansion, so `$\`
// followed by a newline and `{name# #}` opens `${ }`. The number sign after the
// blank is then pattern-removal syntax rather than a comment, and the heredoc on
// the same line is still declared.
#[test]
fn a_line_continuation_before_a_brace_still_opens_the_parameter_expansion() {
    assert_declaration_keeps_its_body(
        &format!(": $\\\n{{name# #}} <<EOF\n{INS_TOKEN}\nEOF\necho {OUTS_TOKEN}"),
        "a continued `${` still opens a parameter expansion",
    );
}

// Verify a comment inside backticks ends at the closing backtick.
//
// Bash reads a backtick body as its own command region, so `# inner` there is a
// comment that ends with the substitution rather than swallowing the rest of
// the outer line. Copying through the closing backtick leaves the traversal
// inside the region, so `<<EOF` is never queued and the body marker fails.
#[test]
fn a_comment_inside_backticks_ends_at_the_closing_backtick() {
    assert_substitution_keeps_the_declaration(
        "`printf x # inner`",
        "a comment inside backticks ends at the closing backtick",
    );
}

// Verify a `case` pattern terminator does not close the command substitution.
//
// The `)` after `x` ends a pattern rather than the substitution, so the region
// stays open until the `)` after `esac`. Closing at the pattern terminator would
// restore the enclosing `${ }` count early, read the following number sign as a
// comment, and hide the heredoc declaration.
#[test]
fn a_case_pattern_terminator_does_not_close_the_substitution_region() {
    assert_substitution_keeps_the_declaration(
        "$(case x in x) printf } ;; esac)",
        "a case pattern terminator keeps the substitution region open",
    );
}
