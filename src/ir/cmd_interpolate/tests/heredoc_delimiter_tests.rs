//! Unit tests for POSIX heredoc delimiter lowering in recipes.

use super::*;

/// Wrap recipe text so the shared helpers take named domain values.
///
/// Keeping the template distinct from its expected lowering stops the two from
/// being passed in the wrong order, which no assertion message could catch.
#[derive(Clone, Copy)]
struct Recipe<'a>(&'a str);

/// The exact text both traversals must produce, and the rule it demonstrates.
struct Lowering<'a> {
    /// The text every traversal must lower the recipe to.
    text: &'a str,
    /// The rule named in a failure message.
    rule: &'a str,
}

/// One heredoc body scenario: a declaration, its terminator, and whether a
/// marker inside the body must stay inert.
struct HeredocCase<'a> {
    /// The declaration text, including the newline that ends it.
    declaration: &'a str,
    /// The line that closes the body.
    terminator: &'a str,
    /// Whether a marker placed in the body must survive lowering.
    is_body_inert: bool,
}

/// The terminator and expected prefix used to prove a body stays inert.
struct InertBody<'a> {
    /// The line that closes the body.
    terminator: &'a str,
    /// The source text that must survive lowering byte-for-byte.
    expected_prefix: &'a str,
}

/// Assert that both POSIX traversals lower `recipe` to exactly `lowering.text`.
///
/// The `command:` and `script:` routes share these heredoc rules, so every case
/// is stated once and checked through both. One set of bindings serves them
/// all: a template that carries no marker never reads the bound paths.
fn assert_both_traversals_lower(recipe: Recipe<'_>, lowering: &Lowering<'_>) {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("input")],
        &[Utf8PathBuf::from("output")],
        RecipeShell::Posix,
    );
    for result in [
        substitute(recipe.0, &bindings),
        substitute_script(recipe.0, &bindings),
    ] {
        let lowered = result.expect(lowering.rule);
        assert_eq!(lowered, lowering.text, "{}", lowering.rule);
    }
}

// Verify a Unicode separator cannot shorten a heredoc delimiter.
//
// The shell keeps a non-blank separator inside the delimiter word, so the
// body stays inert until the full terminator appears. Lowering the marker
// after the fake `EOF` line would place a path inside an expanding heredoc
// body, where backticks in that path remain executable.
#[test]
fn unicode_separator_does_not_shorten_a_heredoc_delimiter() {
    let input_path = "a`touch injected`b";
    let output_path = "x\"; touch injected; echo \"y";
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from(input_path)],
        &[Utf8PathBuf::from(output_path)],
        RecipeShell::Posix,
    );
    let terminator = "EOF\u{00A0}suffix";
    let template = format!(
        "cat <<{terminator}\nbody {INS_TOKEN}\nEOF\n{INS_TOKEN}\n{terminator}\necho \"{OUTS_TOKEN}\""
    );
    let expected_prefix = format!("cat <<{terminator}\nbody {INS_TOKEN}\nEOF\n{INS_TOKEN}\n");
    for lowered in [
        substitute(&template, &bindings),
        substitute_script(&template, &bindings),
    ] {
        assert_heredoc_body_stays_inert(
            lowered,
            &InertBody {
                terminator,
                expected_prefix: &expected_prefix,
            },
        );
    }
}

/// Assert one lowering leaves everything before the terminator untouched.
///
/// The prefix is compared exactly rather than searched for the input path.
/// Lowering into executable text escapes the path, so a `contains` check for
/// the raw path would pass even after the path had been moved into the body;
/// an exact comparison catches any lowering at all, including a lowered
/// marker. Both traversals must reach the same result: the security boundary
/// is the heredoc body's contents, so checking only the script route would let
/// a regression in `SubstitutionTraversal` pass unnoticed.
fn assert_heredoc_body_stays_inert(result: Result<String, IrGenError>, body: &InertBody<'_>) {
    let interpolated = result.expect("a Unicode separator must not end the delimiter early");
    let (inert_body, active) = interpolated
        .rsplit_once(&format!("{}\n", body.terminator))
        .expect("the full terminator must survive lowering");
    assert_eq!(
        inert_body, body.expected_prefix,
        "the heredoc body must be byte-identical to the source"
    );
    assert_eq!(
        active, "echo \"x\\\"; touch injected; echo \\\"y\"",
        "a marker after the real terminator must use quote-aware encoding"
    );
}

// Verify heredoc bodies follow the shell's delimiter and continuation rules.
//
// An unquoted delimiter leaves the body subject to expansion, and the shell
// removes backslash-newline before it looks for the terminator. A physical
// line that looks like the delimiter can therefore be only part of a longer
// logical line. Matching physical lines would end the body early and lower a
// marker into text the shell still expands, where backticks in an inserted
// path remain executable. A quoted delimiter suppresses expansion and joining
// alike, and an even backslash run leaves the newline unescaped, so both keep
// the physical line as the unit.
#[rstest]
#[case::plain(HeredocCase { declaration: "cat <<EOF\n", terminator: "EOF", is_body_inert: true })]
#[case::tab_stripping(HeredocCase { declaration: "cat <<-EOF\n", terminator: "\tEOF", is_body_inert: true })]
#[case::single_quoted(HeredocCase { declaration: "cat <<'EOF'\n", terminator: "EOF", is_body_inert: true })]
#[case::double_quoted(HeredocCase { declaration: "cat <<\"EOF\"\n", terminator: "EOF", is_body_inert: true })]
#[case::mixed_quotes(HeredocCase { declaration: "cat <<E\"O\"F\n", terminator: "EOF", is_body_inert: true })]
#[case::escaped(HeredocCase { declaration: "cat <<\\EOF\n", terminator: "EOF", is_body_inert: true })]
#[case::unicode(HeredocCase { declaration: "cat <<délim\n", terminator: "délim", is_body_inert: true })]
#[case::continued_line_does_not_terminate(HeredocCase { declaration: "cat <<EOF\nprefix\\\nEOF\n", terminator: "EOF", is_body_inert: true })]
#[case::continued_line_completes_the_terminator(HeredocCase { declaration: "cat <<AB\nA\\\nB\n", terminator: "AB", is_body_inert: false })]
#[case::tab_stripping_precedes_the_join(HeredocCase { declaration: "cat <<-AB\n\tA\\\nB\n", terminator: "AB", is_body_inert: false })]
#[case::continuation_line_keeps_its_tabs(HeredocCase { declaration: "cat <<-AB\n\tA\\\n\tB\n", terminator: "AB", is_body_inert: true })]
#[case::quoted_delimiter_does_not_join(HeredocCase { declaration: "cat <<'EOF'\nprefix\\\nEOF\n", terminator: "EOF", is_body_inert: false })]
#[case::even_backslashes_do_not_join(HeredocCase { declaration: "cat <<EOF\nprefix\\\\\nEOF\n", terminator: "EOF", is_body_inert: false })]
fn heredoc_bodies_follow_the_shell(#[case] case: HeredocCase<'_>) {
    let template = format!(
        "{}{INS_TOKEN}\n{}\necho {OUTS_TOKEN}",
        case.declaration, case.terminator
    );
    let body_marker = if case.is_body_inert {
        INS_TOKEN
    } else {
        "input"
    };
    let expected = format!(
        "{}{body_marker}\n{}\necho output",
        case.declaration, case.terminator
    );
    assert_both_traversals_lower(
        Recipe(&template),
        &Lowering {
            text: &expected,
            rule: "heredoc bodies must follow the shell's delimiter and continuation rules",
        },
    );
}

// Verify both POSIX traversals reject delimiter syntax they cannot model.
#[rstest]
#[case::missing(Recipe("cat <<"))]
#[case::missing_before_newline(Recipe("cat <<\nbody"))]
#[case::missing_before_operator(Recipe("cat <<;"))]
#[case::leading_comment(Recipe("cat <<#EOF\nbody\n#EOF"))]
#[case::unclosed_single_quote(Recipe("cat <<'EOF\nbody\nEOF"))]
#[case::unclosed_double_quote(Recipe("cat <<\"EOF\nbody\nEOF"))]
#[case::trailing_backslash(Recipe("cat <<EOF\\"))]
#[case::line_continuation(Recipe("cat <<EO\\\nF\nbody\nEOF"))]
#[case::unquoted_dollar(Recipe("cat <<$DELIM\nbody\n$DELIM"))]
#[case::double_quoted_dollar(Recipe("cat <<\"$DELIM\"\nbody\n$DELIM"))]
#[case::unquoted_backtick(Recipe("cat <<`delim`\nbody\n`delim`"))]
#[case::double_quoted_backtick(Recipe("cat <<\"`delim`\"\nbody\n`delim`"))]
fn unsupported_heredoc_delimiters_are_rejected_in_both_traversals(#[case] recipe: Recipe<'_>) {
    let bindings = CommandBindings::new(&[], &[], RecipeShell::Posix);
    for result in [
        substitute(recipe.0, &bindings),
        substitute_script(recipe.0, &bindings),
    ] {
        let error = result.expect_err("unsupported heredoc delimiter syntax must be rejected");
        assert!(
            matches!(error, IrGenError::InvalidCommand { .. }),
            "expected an invalid-command error, got {error:?}"
        );
        assert!(
            error
                .to_string()
                .contains("unsupported heredoc delimiter syntax"),
            "the diagnostic must name the rejected syntax: {error}"
        );
    }
}

// Verify `<<` inside an arithmetic expansion is not read as a heredoc operator.
#[rstest]
#[case::shift_literal(Recipe("echo $((1 << 4))"))]
#[case::shift_variable(Recipe("echo $((1 << n))"))]
#[case::shift_parameter(Recipe("echo $((1 << $n))"))]
#[case::shift_parenthesized(Recipe("echo $((1 << (n)))"))]
fn arithmetic_shifts_are_not_heredoc_declarations(#[case] recipe: Recipe<'_>) {
    assert_both_traversals_lower(
        recipe,
        &Lowering {
            text: recipe.0,
            rule: "an arithmetic shift must not be parsed as a heredoc operator",
        },
    );
}

// Verify an arithmetic shift does not leave a body pending after its newline.
//
// Reading `<<` as a heredoc operator queues the rest of the arithmetic line as
// a delimiter, so the next line is taken as body text and its marker is never
// lowered. The trailing marker on the following line is the discriminator.
#[test]
fn an_arithmetic_shift_does_not_consume_the_next_line() {
    let template = format!("echo $((1 << n))\necho {OUTS_TOKEN}");
    assert_both_traversals_lower(
        Recipe(&template),
        &Lowering {
            text: "echo $((1 << n))\necho output",
            rule: "an arithmetic shift must not queue a heredoc body",
        },
    );
}

// Verify heredoc tracking resumes after an arithmetic expansion closes.
//
// The arithmetic suspension belongs to one substitution frame. If it outlived
// the closing `))`, a later declaration would go untracked and a marker in its
// body would be lowered into text the shell still expands.
#[test]
fn heredoc_tracking_resumes_after_an_arithmetic_expansion() {
    let template = format!("n=$((1 << 3))\ncat <<EOF\n{INS_TOKEN}\nEOF\necho {OUTS_TOKEN}");
    let expected = format!("n=$((1 << 3))\ncat <<EOF\n{INS_TOKEN}\nEOF\necho output");
    assert_both_traversals_lower(
        Recipe(&template),
        &Lowering {
            text: &expected,
            rule: "heredoc tracking must resume after the expansion",
        },
    );
}

// Verify `<<<` is copied as a here-string operator without queuing a body.
#[test]
fn here_string_operators_do_not_queue_a_heredoc() {
    let template = format!("cat <<<{INS_TOKEN}\necho \"{OUTS_TOKEN}\"");
    assert_both_traversals_lower(
        Recipe(&template),
        &Lowering {
            text: "cat <<<input\necho \"output\"",
            rule: "a here-string must not be treated as a heredoc",
        },
    );
}
