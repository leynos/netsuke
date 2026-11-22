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

// Verify markers in a case arm stay inside a command substitution.
//
// A `)` that terminates a pattern must not pop the tracked substitution, so a
// marker in the arm is refused rather than lowered as outer text. Popping early
// would encode it for the enclosing quote context while the shell still reads
// it as substitution source. Each case is a way the tracker could lose the
// construct: a pattern terminator, the enclosing double quotes, Bash's optional
// pattern opener, a brace group, and a function body inside the substitution.
// The refusal is an invalid command rather than an ambiguous boundary, because
// nothing about the closer is unprovable: the substitution simply never closes
// where the recipe needs it to.
#[rstest]
#[case::pattern_terminator(
    "echo $(case x in x) {INS_TOKEN} ;; esac)",
    "a marker inside the case body must stay in the substitution",
    Interpolators::Both
)]
#[case::outer_quotes(
    "echo \"$(case x in x) echo {INS_TOKEN} ;; esac)\"",
    "a marker in an outer-quoted case arm must stay protected",
    Interpolators::Both
)]
#[case::pattern_opener(
    "echo \"$(case esac in (esac) printf %s {INS_TOKEN} ;; esac)\"",
    "a marker in a parenthesised case arm must stay protected",
    Interpolators::Both
)]
#[case::brace_group(
    "echo \"$( {{ case x in x) echo {INS_TOKEN} ;; esac; }} )\"",
    "a marker in a brace-grouped case arm must stay protected",
    Interpolators::Both
)]
#[case::function_body(
    "echo \"$( helper() {{ case x in x) echo {INS_TOKEN} ;; esac; }}; helper )\"",
    "a marker in a function-scoped case arm must stay protected",
    Interpolators::ScriptOnly
)]
#[case::in_subject(
    "echo \"$(case in in (in) printf %s {INS_TOKEN} ;; esac)\"",
    "a marker in an arm whose subject is spelled in must stay protected",
    Interpolators::Both
)]
#[case::quoted_subject(
    "echo \"$(case \"x\" in (x) printf %s {INS_TOKEN} ;; esac)\"",
    "a marker in an arm whose subject is quoted must stay protected",
    Interpolators::Both
)]
fn case_arm_markers_inside_substitutions_are_refused(
    #[case] template: &str,
    #[case] context: &str,
    #[case] interpolators: Interpolators,
) {
    let expanded = template.replace("{INS_TOKEN}", INS_TOKEN);
    let bindings = hostile_posix_bindings();
    for interpolate in interpolators.select() {
        let error = interpolate(&expanded, &bindings).expect_err(context);
        assert!(
            matches!(error, IrGenError::InvalidCommand { .. }),
            "the marker must not be lowered for the outer quote context: {error:?}"
        );
    }
}

// Verify supported subjects do not move the tracker into the patterns.
//
// The header is `case WORD in`, so the first word after `case` is the subject
// whatever it spells. Reading it as the keyword starts the pattern list one
// word early: the optional `(` then looks like a group, the arm never enters
// its command phase, and the construct stays open past the real substitution
// closer, so a marker written after the substitution is wrongly refused. A
// subject spelled `in` is one such word; a quoted or empty-quoted subject is
// another, because quoted characters never enter the word buffer.
#[rstest]
#[case::in_subject("case in in (in) printf yes ;; esac)")]
#[case::quoted_subject("case \"x\" in (x) printf yes ;; esac)")]
#[case::quoted_in_subject("case \"in\" in (in) printf yes ;; esac)")]
#[case::mixed_quoted_subject("case x\"y\" in (x) printf yes ;; esac)")]
#[case::empty_quoted_subject("case \"\" in (x) printf yes ;; esac)")]
fn supported_case_subjects_keep_the_substitution_boundary(#[case] substitution: &str) {
    let bindings = bindings();
    let template = format!("echo \"$({substitution}\" {INS_TOKEN}");
    let expected = format!("echo \"$({substitution}\" input");
    for interpolate in both_interpolators() {
        assert_eq!(
            interpolate(&template, &bindings).expect("a quoted subject is a supported case header"),
            expected,
            "a supported case substitution must keep its text and lower the outer marker"
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

// Verify the guard still fires deep inside a long recipe.
//
// The traversal must find the ambiguous closer in the source text, wherever it
// sits. Deciding from a bounded prefix, or from whatever text a diagnostic
// chose to repeat, would accept this recipe and encode a later marker for the
// wrong context.
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
// The ambiguity diagnostic carries the one-based character offset rather than
// any recipe text: the rejected text is exactly the text the scanner could not
// classify, and copying it into a log line would reproduce whatever shell
// syntax made the boundary ambiguous. The position must still be exact deep
// inside a long recipe, where a bounded prefix could not have reported it.
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

// Verify a brace-grouped case substitution without a marker is accepted.
//
// Recognizing `{` as a command-position word must not reject the group form
// itself. The substitution here has no marker and its real closer is not
// followed by `#`, so its text must survive interpolation unchanged.
#[test]
fn brace_grouped_case_substitution_without_markers_is_accepted() {
    let template = "echo \"$( { case x in x) printf y ;; esac; } )\"";
    let bindings = bindings();
    for interpolate in both_interpolators() {
        assert_eq!(
            interpolate(template, &bindings)
                .expect("a case inside a brace group is supported shell syntax"),
            template,
            "a brace-grouped case substitution must be left unchanged: {template}"
        );
    }
}

// Verify a marker after a brace-grouped case substitution is still lowered.
//
// The group form must keep the substitution open only while the case runs. A
// marker after the substitution's own closer is ordinary outer text, so the
// tracker must not leave the construct open past the real `)`.
#[test]
fn marker_after_a_brace_grouped_case_substitution_is_lowered() {
    let template = format!("echo \"$( {{ case x in x) printf y ;; esac; }} )\" {OUTS_TOKEN}");
    let expected = "echo \"$( { case x in x) printf y ;; esac; } )\" output";
    for interpolate in both_interpolators() {
        assert_eq!(
            interpolate(&template, &bindings())
                .expect("a marker after the grouped substitution must be lowered"),
            expected,
            "a marker after the grouped substitution must lower to its path"
        );
    }
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
// weave: run 'weave explain src/ir/cmd_interpolate/posix_lexical/tests/case/boundary.rs' for per-hunk detail, 'weave check' to verify your resolution
