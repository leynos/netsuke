//! Unit tests for the `case` tracker's keyword and comment state.
//!
//! The tracker has to decide whether a word is the `case` or `esac` reserved
//! word, and it only sees characters the traversal offers it. These cases pin
//! the spellings it must recognise, the comments whose text it must never read,
//! and the command position that decides whether a reserved word counts.

use super::super::*;

// Verify a keyword spelled across a line continuation is recognised.
//
// The shell removes an unquoted backslash-newline pair before tokenising, so
// `c\<newline>ase` is the `case` reserved word. A tracker that ended the word
// at either character would read the pattern `)` as the substitution closer,
// let the following `#` start a comment, and encode a later marker unquoted.
#[test]
fn case_keyword_spelled_with_line_continuation_fails_closed() {
    let template = format!("echo $(c\\\nase x in x) :;; esac)#{INS_TOKEN}");
    assert_both_reject(
        &template,
        "a keyword continued across a newline must still fail closed",
    );
}

// Verify a keyword after a long run of continuations is still recognised.
//
// The raw-source scan runs the keyword test at every position it reads, so a
// test that skipped continuations before comparing the first character would
// walk the rest of the run from each position inside it and make the
// fail-closed check quadratic. Rejecting a position that does not spell the
// first character keeps each candidate constant-time, while `c` followed by any
// number of pairs and `ase` is still the keyword the shell builds.
#[test]
fn keyword_after_a_long_continuation_run_is_recognised() {
    let run = "\\\n".repeat(4096);
    let template =
        format!("echo \"$(coproc \"jobs\" c{run}ase x in x) echo {INS_TOKEN} ;; esac)\"");
    for interpolate in both_interpolators() {
        let error = interpolate(&template, &hostile_bash_bindings())
            .expect_err("a continued keyword after a long run must fail closed");
        assert!(
            matches!(error, IrGenError::InvalidCommand { .. }),
            "the marker must not be lowered for the outer quote context: {error:?}"
        );
    }
}

// Verify a keyword spelled with backslash escapes is recognised.
//
// An unquoted backslash removes the special meaning of the escaped character
// and contributes it literally, so `ca\se` is the `case` reserved word. The
// same fail-closed rule applies as for the plainly spelled keyword.
#[test]
fn case_keyword_spelled_with_escape_fails_closed() {
    let template = format!("echo $(ca\\se x in x) :;; esac)#{INS_TOKEN}");
    assert_both_reject(
        &template,
        "an escaped spelling of the keyword must still fail closed",
    );
}

// Verify a `case` inside a comment does not open a construct.
//
// The `#` follows a space inside the substitution, so the shell reads the rest
// of the line as a comment and the `case` in it is ordinary text. Treating it
// as a keyword would leave the construct open past the substitution's real
// closer and reject a supported recipe.
#[test]
fn case_in_a_comment_does_not_open_a_construct() {
    let template = format!("echo $(printf x # case\n) y {INS_TOKEN}");
    let expected = "echo $(printf x # case\n) y input";
    for interpolate in both_interpolators() {
        assert_eq!(
            interpolate(&template, &bindings()).expect("a commented 'case' is not a keyword"),
            expected,
            "a commented 'case' must not open a construct"
        );
    }
}

// Verify a word that merely contains `case` is not treated as the keyword.
//
// `showcase` ends in the same four letters as the `case` reserved word, and
// the `)` that closes its substitution is followed by `#`. A substring match
// would reject this supported recipe; only a whole-word match may fail
// closed. Because the closer stays in the word, the `#` is word data and the
// quote after it still opens a double-quoted region.
#[test]
fn word_containing_case_before_hash_interpolates_normally() {
    let (template, expected) = executable_quote_case("echo $(showcase)");
    assert_output(
        &template,
        &expected,
        "a word containing 'case' must not trigger the ambiguity guard",
        Interpolators::Both,
    );
}

// Verify a word that merely starts with `case` is not treated as the keyword.
//
// `casefile` begins with the same four letters as the reserved word, and the
// `)` that closes its substitution is followed by `#`. Only a whole-word match
// may fail closed, so this supported recipe must interpolate normally.
#[test]
fn word_starting_with_case_before_hash_interpolates_normally() {
    let (template, expected) = executable_quote_case("echo $(casefile)");
    assert_output(
        &template,
        &expected,
        "a word starting with 'case' must not trigger the ambiguity guard",
        Interpolators::Both,
    );
}

// Verify an unquoted PowerShell substitution boundary still fails closed.
//
// PowerShell has no `case` patterns, so the ambiguity does not arise there and
// the guard keeps its narrower unquoted-only rule. An unquoted `$(...)` whose
// source contains `case` and whose closer is followed by `#` must still be
// rejected, because the raw-source scan cannot tell a keyword from word data.
#[test]
fn unquoted_power_shell_case_boundary_fails_closed() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("input")],
        &[Utf8PathBuf::from("output")],
        RecipeShell::PowerShell,
    );
    let template = format!("Write-Output $(Write-Output case)#{INS_TOKEN}");

    for interpolate in both_interpolators() {
        let error = interpolate(&template, &bindings)
            .expect_err("an unquoted PowerShell case boundary must fail closed");
        assert!(
            matches!(error, IrGenError::AmbiguousSubstitutionBoundary { .. }),
            "unexpected error: {error:?}"
        );
    }
}

// Verify a comment newline does not hide the `case` that follows it.
//
// The traversal copies a comment's text and its terminating newline as inert
// before the `case` tracker sees either. If the newline never reaches the
// tracker, the tracker stays inside the comment for the rest of the
// substitution: the `case` below opens nothing, the group closer is read as
// the substitution closer, and the real `)#` boundary is then mistaken for a
// comment. The marker on the next line would be encoded unquoted while the
// shell has the double quote open.
#[test]
fn case_after_a_comment_newline_fails_closed() {
    let (template, _) = executable_quote_case("echo $( ( # note\ncase x in x) :;; esac))");
    assert_both_reject(
        &template,
        "a case after a comment newline must fail closed at the real boundary",
    );
}

// Verify a comment right after an inner-group closer stays literal.
//
// The `)` here closes a subshell inside the substitution, so it is an
// operator and `#` after it starts a comment. Comment text is copied verbatim,
// so the marker spelled inside it must survive unchanged, and the marker after
// the real closer must still be encoded for the double-quoted region that
// follows. A scanner that ended the comment early would lower the first marker
// and encode the second for the wrong context.
#[test]
fn comment_after_an_inner_group_closer_stays_literal() {
    let template = format!("echo $( (printf x)# note {INS_TOKEN}\n) \"{INS_TOKEN}\"");
    let expected = format!("echo $( (printf x)# note {INS_TOKEN}\n) \"{HOSTILE_DOUBLE_QUOTED}\"");
    assert_output(
        &template,
        &expected,
        "a comment after an inner-group closer must stay inert text",
        Interpolators::Both,
    );
}

// Verify `esac` used as a command argument does not close the construct.
//
// A reserved word is recognised only in command position, so `printf esac`
// passes `esac` as an argument. Popping the tracker there ends the construct
// early, and the later pattern terminator is then read as the substitution
// closer one `)` too soon, which lets the real `)#` read as a comment.
#[test]
fn esac_used_as_a_command_argument_fails_closed() {
    let (template, _) = executable_quote_case("echo $( (case x in x) printf esac;; y) :;; esac))");
    assert_both_reject(
        &template,
        "an 'esac' argument must not close the construct before the real boundary",
    );
}

// Verify a `;;` split by a line continuation still ends the arm.
//
// Verify the pattern-list grammar keeps the substitution boundary intact.
//
// Each rejected row is a way the tracker could misread an operator. Bash
// deletes an unquoted backslash-newline pair before tokenising, so `;` + a
// continuation + `;` is the `;;` arm terminator: a tracker that records the
// backslash never sees the pair, reads the next pattern list as arm text, lets
// the `esac` in that list close the construct, and treats the following pattern
// terminator as the substitution closer. Bash also accepts an optional `(`
// before a pattern list, so `(x)` is the pattern `x`: counting that `(` as a
// group reads the terminator at a deeper depth, where it closes nothing, and
// leaves the construct in its pattern phase. Each accepted row holds the same
// grammar without a marker, which is supported Bash and must keep its text.
#[rstest]
#[case::continued_terminator(
    "echo \"$(case x in a) : ;\\\n; x|esac) printf %s {INS_TOKEN} ;; esac)\"",
    Some("a `;;` split by a continuation must still end the arm"),
    "a `;;` split by a continuation is supported Bash"
)]
#[case::optional_opener(
    "echo \"$(case z in (x) case y in y) : ;; esac ;; z) printf %s {INS_TOKEN} ;; esac)\"",
    Some("an optional pattern opener must not hide the nested construct"),
    "Bash accepts an optional pattern opener"
)]
#[case::continued_terminator_without_markers(
    "echo \"$(case x in a) : ;\\\n; esac)\"",
    None,
    "a `;;` split by a continuation is supported Bash"
)]
#[case::optional_opener_without_markers(
    "echo \"$(case z in (x) : ;; esac)\"",
    None,
    "Bash accepts an optional pattern opener"
)]
fn pattern_list_grammar_keeps_the_substitution_boundary(
    #[case] template: &str,
    #[case] rejection: Option<&str>,
    #[case] acceptance: &str,
) {
    let expanded = template.replace("{INS_TOKEN}", INS_TOKEN);
    let hostile = hostile_posix_bindings();
    for interpolate in both_interpolators() {
        let Some(context) = rejection else {
            assert_eq!(
                interpolate(&expanded, &bindings()).expect(acceptance),
                expanded,
                "a supported `case` substitution must keep its text"
            );
            continue;
        };
        let error = interpolate(&expanded, &hostile).expect_err(context);
        assert!(
            matches!(error, IrGenError::InvalidCommand { .. }),
            "the marker must not be lowered for the outer quote context: {error:?}"
        );
        assert!(
            interpolate(&expanded, &bindings()).is_err(),
            "a marker inside the substitution must stay inside it"
        );
    }
}
