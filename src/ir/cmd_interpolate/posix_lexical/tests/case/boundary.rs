//! Unit tests for `case` pattern boundaries inside command substitutions.
//!
//! A `)` that terminates a `case` pattern is an operator, while the `)` that
//! closes `$()` stays part of the surrounding word. Without the `case` grammar
//! the two are lexically identical, so these cases pin both the accepted shape
//! and the boundary that still fails closed.

use super::super::*;

use rstest::rstest;

// Verify the migration remedies are accepted with their text intact.
//
// The migration guide tells affected users either to separate a comment from
// the closer with whitespace, to keep a literal `#` in a quoted region, or to
// escape it as `\#`. A guide that recommended a recipe the scanner still
// rejects would strand every user who followed it, and a remedy that changed
// the recipe's output would silently rewrite what the recipe prints. Each
// documented form is therefore pinned by its exact interpolated text, for both
// interpolators.
#[rstest]
#[case::whitespace_before_comment("echo \"$(case x in x) :;; esac)\" # trailing")]
#[case::quoted_literal_hash(r#"echo "$(case x in x) :;; esac)"'# trailing comment'"#)]
#[case::escaped_literal_hash("echo $(case x in x) :;; esac)\\#literal")]
fn migration_remedies_preserve_the_recipe_text(#[case] template: &str) {
    let bindings = bindings();
    for interpolate in both_interpolators() {
        assert_eq!(
            interpolate(template, &bindings).expect("the documented remedy must be accepted"),
            template,
            "the documented remedy must leave the recipe unchanged: {template}"
        );
    }
}

// Verify the recipe the migration guide presents as rejected is rejected.
//
// The guide's worked example is the shape users are told will stop working, so
// the warning is only accurate while that exact text still fails closed.
#[test]
fn migration_guide_trigger_recipe_is_rejected() {
    let template = "echo \"$(case x in x) :;; esac)# trailing comment\"";
    assert_both_reject(
        template,
        "the recipe the migration guide presents as rejected must fail closed",
    );
}

// Verify a complete `case` substitution without a comment boundary is accepted.
//
// The pattern terminator is not the substitution closer, so the traversal must
// keep the substitution open and lower the marker that follows the real one.
// An implementation that popped at the pattern terminator would treat the rest
// of the case body as outer text and encode the marker for the wrong context.
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

// Verify a marker after the pattern terminator stays inside the substitution.
//
// The `)` that ends `x)` must not pop the tracked substitution, so the marker
// in the case body is still refused rather than lowered as outer text. Popping
// early would encode it unquoted while the shell treats it as substitution
// source. The refusal is an invalid command rather than an ambiguous boundary,
// because nothing about this closer is unprovable: the substitution simply
// never closes where the recipe needs it to.
#[test]
fn case_body_marker_after_pattern_terminator_is_refused() {
    let template = format!("echo $(case x in x) {INS_TOKEN} ;; esac)");
    let bindings = hostile_posix_bindings();
    for interpolate in both_interpolators() {
        let error = interpolate(&template, &bindings)
            .expect_err("a marker inside the case body must stay in the substitution");
        assert!(
            matches!(error, IrGenError::InvalidCommand { .. }),
            "the marker must not be lowered as outer text: {error:?}"
        );
    }
}

// Verify a `case` substitution whose real closer precedes `#` fails closed.
//
// The `)` after `esac` closes the substitution and so stays in the word, but
// the guard scans raw source and cannot prove the reserved words are keywords.
// It rejects rather than risk encoding a later marker for the wrong context.
#[test]
fn case_substitution_real_closer_before_hash_fails_closed() {
    let template = format!("echo $(case x in x) :;; esac)#{INS_TOKEN}");
    assert_both_reject(
        &template,
        "an ambiguous case boundary at the real substitution closer must fail closed",
    );
}

// Verify the ambiguous closer keeps its quote context on a later executable line.
//
// The `)` before `#` is the substitution's real closer, so it stays in the
// surrounding word. A scanner that instead read `#` as a comment start would
// leave the following double quote inert and encode the marker on the next
// line for the wrong context. The guard must fail closed before that encoding,
// through both interpolators.
#[test]
fn case_real_closer_preserves_quote_context_by_rejecting() {
    let (template, _) = executable_quote_case("echo $(case x in x) :;; esac)");
    assert_both_reject(
        &template,
        "an ambiguous case boundary must fail closed instead of flipping the quote context",
    );
}

// Verify the guard still fires beyond the truncated diagnostic snippet.
//
// The invalid-command diagnostic quotes only the first 160 characters of the
// recipe, so a boundary that sits later must still be found by the traversal.
// Reading the boundary from the truncated message instead of the source would
// accept this recipe and encode a later marker for the wrong context.
#[test]
fn case_substitution_real_closer_beyond_diagnostic_snippet_fails_closed() {
    let padding = format!("echo {} $(case x in x) :;; esac)", "x".repeat(200));
    let (template, _) = inert_comment_case(&padding, None);
    assert_both_reject(
        &template,
        "an ambiguous case boundary beyond the snippet must fail closed",
    );
}

// Verify the boundary diagnostic reports a location instead of the recipe.
//
// The invalid-command diagnostic quotes only the first 160 characters, so a
// longer recipe loses the boundary it is trying to report. The ambiguity
// diagnostic instead carries the one-based character offset, and it must not
// quote the recipe at all: the rejected text is exactly the text the scanner
// could not classify, and copying it into a log line would reproduce whatever
// shell syntax made the boundary ambiguous.
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

// Verify the ambiguity guard fires for every POSIX-family script shell.
//
// POSIX and Bash scripts share one traversal, so the rule must not depend on
// which of the two selected it. Both spell `case` the same way, and the shell
// that reads the finished script applies the same pattern rule.
#[rstest]
#[case::posix(RecipeShell::Posix)]
#[case::bash(RecipeShell::Bash)]
fn ambiguous_case_boundary_fails_closed_for_each_script_shell(#[case] shell: RecipeShell) {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("input")],
        &[Utf8PathBuf::from("output")],
        shell,
    );
    let template = format!("echo $(case x in x) :;; esac)#{INS_TOKEN}");
    let error = interpolate_script_with_bindings(&template, &bindings)
        .expect_err("an ambiguous case boundary must fail closed for every script shell");
    assert!(
        matches!(error, IrGenError::AmbiguousSubstitutionBoundary { .. }),
        "unexpected error: {error:?}"
    );
}

// Verify an outer-quoted substitution is still checked for the `case` ambiguity.
//
// The enclosing double quotes do not extend into `$(...)`: the shell parses a
// substitution as its own script, so a real `)#` closer is just as ambiguous
// as an unquoted one. Deciding from the quote context read after the
// substitution is popped would skip the rejection and let a `#` comment inside
// the substitution flip the scanner's quote state.
#[test]
fn case_substitution_inside_outer_quotes_fails_closed() {
    let template = format!("echo \"$(case x in x) :;; esac)#{OUTS_TOKEN}\"");
    assert_both_reject(
        &template,
        "an outer-quoted case substitution closer before '#' must fail closed",
    );
}

// Verify a `case` inside a nested group still finds its pattern terminator.
//
// `case` opens at the parenthesis depth of the group that contains it, so the
// `)` that ends its pattern is one level below the substitution. Matching a
// terminator only at the substitution's own depth reads that `)` as closing
// the group: the following `esac` then closes nothing, and the next `)` is
// taken for the substitution closer one `)` too early. The leftover `)#` is
// then read as a comment, which leaves the quote opened after it inert and
// encodes the next line's marker for the wrong context.
#[test]
fn case_pattern_terminator_inside_nested_group_fails_closed() {
    let (template, _) = executable_quote_case("echo $( (case x in x) :;; esac))");
    assert_both_reject(
        &template,
        "a case pattern inside a nested group must fail closed before the boundary",
    );
}

// Verify the raw-source scan rejects a `case` spelled inside a quoted string.
//
// The guard deliberately does not re-lex the substitution, so a whole-word
// `case` between separator characters is enough to fail closed even when the
// shell would read it as ordinary string data. Narrowing the scan would mean
// parsing the substitution a second time, which is the ambiguity the guard
// exists to avoid, so the over-approximation is pinned here.
#[test]
fn case_spelled_inside_a_quoted_string_fails_closed() {
    let (template, _) = executable_quote_case("echo $(echo \" case \")");
    assert_both_reject(
        &template,
        "a whole-word 'case' inside a quoted string must still fail closed",
    );
}

// Verify the raw-source scan rejects a `case` spelled inside a comment.
//
// As with the quoted spelling, the guard does not skip comments while it
// scans. The `case` here is comment text to the shell, but a `)` immediately
// before `#` still leaves the closer unprovable, so the recipe fails closed
// rather than encode a later marker for a guessed context.
#[test]
fn case_spelled_inside_a_comment_fails_closed() {
    let (template, _) = executable_quote_case("echo $(echo hi # case x in x\n)");
    assert_both_reject(
        &template,
        "a whole-word 'case' inside a comment must still fail closed",
    );
}
