//! Unit tests for `case` pattern boundaries inside command substitutions.
//!
//! A `)` that terminates a `case` pattern is an operator, while the `)` that
//! closes `$()` stays part of the surrounding word. Without the `case` grammar
//! the two are lexically identical, so these cases pin both the accepted shape
//! and the boundary that still fails closed.

use super::*;

use rstest::rstest;

/// Verify the migration remedies for an ambiguous `)#` boundary are accepted.
///
/// The migration guide tells affected users either to separate a comment from
/// the closer with whitespace or to escape a literal `#` as `\#`. A guide that
/// recommended a recipe the scanner still rejects would strand every user who
/// followed it, so both forms are pinned here for both interpolators.
#[rstest]
#[case::whitespace_before_comment("echo \"$(case x in x) :;; esac)\" # trailing")]
#[case::escaped_literal_hash("echo $(printf x)\\#literal")]
fn migration_remedies_for_ambiguous_boundary_are_accepted(#[case] template: &str) {
    let bindings = bindings();
    for interpolate in both_interpolators() {
        assert!(
            interpolate(template, &bindings).is_ok(),
            "the documented remedy must be accepted: {template}"
        );
    }
}

/// Verify a complete `case` substitution without a comment boundary is accepted.
///
/// The pattern terminator is not the substitution closer, so the traversal must
/// keep the substitution open and lower the marker that follows the real one.
/// An implementation that popped at the pattern terminator would treat the rest
/// of the case body as outer text and encode the marker for the wrong context.
#[test]
fn case_substitution_without_comment_boundary_is_accepted() {
    let bindings = bindings();
    let template = format!("echo $(case x in x) :;; esac) y {INS_TOKEN}");
    let expected = "echo $(case x in x) :;; esac) y input";
    for interpolate in both_interpolators() {
        assert_eq!(
            interpolate(&template, &bindings)
                .expect("a complete case substitution without ')#' must be accepted"),
            expected,
            "a complete case substitution without a comment boundary must be accepted"
        );
    }
}

/// Verify a marker after the pattern terminator stays inside the substitution.
///
/// The `)` that ends `x)` must not pop the tracked substitution, so the marker
/// in the case body is still refused rather than lowered as outer text. Popping
/// early would encode it unquoted while the shell treats it as substitution
/// source.
#[test]
fn case_body_marker_after_pattern_terminator_is_refused() {
    let template = format!("echo $(case x in x) {INS_TOKEN} ;; esac)");
    assert_both_reject(
        &template,
        "a marker inside the case body must stay in the substitution",
    );
}

/// Verify a `case` substitution whose real closer precedes `#` fails closed.
///
/// The `)` after `esac` closes the substitution and so stays in the word, but
/// the guard scans raw source and cannot prove the reserved words are keywords.
/// It rejects rather than risk encoding a later marker for the wrong context.
#[test]
fn case_substitution_real_closer_before_hash_fails_closed() {
    let template = format!("echo $(case x in x) :;; esac)#{INS_TOKEN}");
    assert_both_reject(
        &template,
        "an ambiguous case boundary at the real substitution closer must fail closed",
    );
}

/// Verify the guard still fires beyond the truncated diagnostic snippet.
///
/// The invalid-command diagnostic quotes only the first 160 characters of the
/// recipe, so a boundary that sits later must still be found by the traversal.
/// Reading the boundary from the truncated message instead of the source would
/// accept this recipe and encode a later marker for the wrong context.
#[test]
fn case_substitution_real_closer_beyond_diagnostic_snippet_fails_closed() {
    let padding = format!("echo {} $(case x in x) :;; esac)", "x".repeat(200));
    let (template, _) = inert_comment_case(&padding, None);
    assert_both_reject(
        &template,
        "an ambiguous case boundary beyond the snippet must fail closed",
    );
}

/// Verify the boundary diagnostic reports a location instead of the recipe.
///
/// The invalid-command diagnostic quotes only the first 160 characters, so a
/// longer recipe loses the boundary it is trying to report. The ambiguity
/// diagnostic instead carries the one-based character offset, and it must not
/// quote the recipe at all: the rejected text is exactly the text the scanner
/// could not classify, and copying it into a log line would reproduce whatever
/// shell syntax made the boundary ambiguous.
#[test]
fn ambiguous_boundary_diagnostic_reports_position_past_the_snippet_limit() {
    let padding = format!("echo {} $(case x in x) :;; esac)", "x".repeat(200));
    let (template, _) = inert_comment_case(&padding, None);
    let boundary = template
        .find(")#")
        .expect("the recipe contains the ambiguous boundary")
        + 1;
    let bindings = hostile_posix_bindings();

    for interpolate in both_interpolators() {
        let error = interpolate(&template, &bindings)
            .expect_err("an ambiguous boundary past the snippet limit must fail closed");
        let IrGenError::AmbiguousSubstitutionBoundary { position, message } = error else {
            panic!("unexpected error: {error:?}");
        };
        assert_eq!(
            position, boundary,
            "the diagnostic must locate the ambiguous ')' rather than truncate it"
        );
        let rendered = message.to_string();
        assert!(
            rendered.contains(&boundary.to_string()),
            "the rendered diagnostic must name the boundary position: {rendered}"
        );
        assert!(
            !rendered.contains("esac") && !rendered.contains("echo"),
            "the rendered diagnostic must not quote the rejected recipe: {rendered}"
        );
    }
}

/// Verify a keyword spelled across a line continuation is recognised.
///
/// The shell removes an unquoted backslash-newline pair before tokenising, so
/// `c\<newline>ase` is the `case` reserved word. A tracker that ended the word
/// at either character would read the pattern `)` as the substitution closer,
/// let the following `#` start a comment, and encode a later marker unquoted.
#[test]
fn case_keyword_spelled_with_line_continuation_fails_closed() {
    let template = format!("echo $(c\\\nase x in x) :;; esac)#{INS_TOKEN}");
    assert_both_reject(
        &template,
        "a keyword continued across a newline must still fail closed",
    );
}

/// Verify a keyword spelled with backslash escapes is recognised.
///
/// An unquoted backslash removes the special meaning of the escaped character
/// and contributes it literally, so `ca\se` is the `case` reserved word. The
/// same fail-closed rule applies as for the plainly spelled keyword.
#[test]
fn case_keyword_spelled_with_escape_fails_closed() {
    let template = format!("echo $(ca\\se x in x) :;; esac)#{INS_TOKEN}");
    assert_both_reject(
        &template,
        "an escaped spelling of the keyword must still fail closed",
    );
}

/// Verify a `case` inside a comment does not open a construct.
///
/// The `#` follows a space inside the substitution, so the shell reads the rest
/// of the line as a comment and the `case` in it is ordinary text. Treating it
/// as a keyword would leave the construct open past the substitution's real
/// closer and reject a supported recipe.
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

/// Verify a word that merely contains `case` is not treated as the keyword.
///
/// `showcase` ends in the same four letters as the `case` reserved word, and
/// the `)` that closes its substitution is followed by `#`. A substring match
/// would reject this supported recipe; only a whole-word match may fail
/// closed. Because the closer stays in the word, the `#` is word data and the
/// quote after it still opens a double-quoted region.
#[test]
fn word_containing_case_before_hash_interpolates_normally() {
    let (template, expected) = executable_quote_case("echo $(showcase)");
    assert_both_outputs(
        &template,
        &expected,
        "a word containing 'case' must not trigger the ambiguity guard",
    );
}

/// Verify a word that merely starts with `case` is not treated as the keyword.
///
/// `casefile` begins with the same four letters as the reserved word, and the
/// `)` that closes its substitution is followed by `#`. Only a whole-word match
/// may fail closed, so this supported recipe must interpolate normally.
#[test]
fn word_starting_with_case_before_hash_interpolates_normally() {
    let (template, expected) = executable_quote_case("echo $(casefile)");
    assert_both_outputs(
        &template,
        &expected,
        "a word starting with 'case' must not trigger the ambiguity guard",
    );
}

/// Verify an outer-quoted substitution is still checked for the `case` ambiguity.
///
/// The enclosing double quotes do not extend into `$(...)`: the shell parses a
/// substitution as its own script, so a real `)#` closer is just as ambiguous
/// as an unquoted one. Deciding from the quote context read after the
/// substitution is popped would skip the rejection and let a `#` comment inside
/// the substitution flip the scanner's quote state.
#[test]
fn case_substitution_inside_outer_quotes_fails_closed() {
    let template = format!("echo \"$(case x in x) :;; esac)#{OUTS_TOKEN}\"");
    assert_both_reject(
        &template,
        "an outer-quoted case substitution closer before '#' must fail closed",
    );
}
