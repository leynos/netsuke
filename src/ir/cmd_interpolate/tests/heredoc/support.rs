//! Shared helpers for the POSIX heredoc lowering tests.
//!
//! Every heredoc rule is stated once and checked through both recipe
//! traversals, so the helpers that build bindings and compare lowerings live
//! here rather than in one of the test modules that use them.

use super::super::*;

/// Wrap recipe text so the shared helpers take named domain values.
///
/// Keeping the template distinct from its expected lowering stops the two from
/// being passed in the wrong order, which no assertion message could catch.
#[derive(Clone, Copy)]
pub(super) struct Recipe<'a>(pub(super) &'a str);

/// Path whose unquoted encoding differs from its double-quoted encoding.
///
/// The semicolon is safe only when the unquoted encoder has closed the word
/// with single quotes. The double-quoted encoding escapes quotes, dollars,
/// backticks, and backslashes but leaves a semicolon bare, so inserting that
/// form outside quotes would run the embedded commands.
pub(super) const HOSTILE_PATH: &str = "x\"; touch injected; echo \"y";

/// The encoding an unquoted insertion site requires for [`HOSTILE_PATH`].
pub(super) const HOSTILE_UNQUOTED: &str = "x'\"; touch injected; echo \"y'";

/// The exact text both traversals must produce, and the rule it demonstrates.
pub(super) struct Lowering<'a> {
    /// The text every traversal must lower the recipe to.
    pub(super) text: &'a str,
    /// The rule named in a failure message.
    pub(super) rule: &'a str,
}

/// Assert that both POSIX traversals lower `recipe` to exactly `lowering.text`.
///
/// The `command:` and `script:` routes share these heredoc rules, so every case
/// is stated once and checked through both. One set of bindings serves them
/// all: a template that carries no marker never reads the bound paths.
pub(super) fn assert_both_traversals_lower(recipe: Recipe<'_>, lowering: &Lowering<'_>) {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("input")],
        &[Utf8PathBuf::from("output")],
        RecipeShell::Posix,
    );
    assert_both_traversals_lower_with(recipe, &bindings, lowering);
}

/// Assert a declared heredoc keeps its body token and lowers the later marker.
///
/// The expectation is derived from the template rather than retyped beside it,
/// so the recipe is stated once and the two can never drift apart. Keeping the
/// input token in the body is the whole point of the case: a tracker that lost
/// the declaration would substitute the bound path there instead.
pub(super) fn assert_declaration_keeps_its_body(template: &str, rule: &str) {
    let expected = template.replace(OUTS_TOKEN, "output");
    assert_both_traversals_lower(
        Recipe(template),
        &Lowering {
            text: &expected,
            rule,
        },
    );
}

/// Assert both traversals lower `recipe` to `lowering.text` with `bindings`.
///
/// Cases whose result depends on the bound paths supply their own bindings;
/// the shared default above covers the rest.
pub(super) fn assert_both_traversals_lower_with(
    recipe: Recipe<'_>,
    bindings: &CommandBindings,
    lowering: &Lowering<'_>,
) {
    for result in [
        substitute(recipe.0, bindings),
        substitute_script(recipe.0, bindings),
    ] {
        let lowered = result.expect(lowering.rule);
        assert_eq!(lowered, lowering.text, "{}", lowering.rule);
    }
}

/// Build bindings whose paths expose a wrong quote-context encoding.
///
/// Both positions carry the hostile path so a marker placed after a heredoc
/// body reveals whether the traversal still believes the shell is inside the
/// multiline argument's quotes.
pub(super) fn hostile_bindings() -> CommandBindings {
    CommandBindings::new(
        &[Utf8PathBuf::from(HOSTILE_PATH)],
        &[Utf8PathBuf::from(HOSTILE_PATH)],
        RecipeShell::Posix,
    )
}

/// Assert both traversals reject a heredoc boundary they cannot place safely.
///
/// The rejection must use the dedicated heredoc diagnostic rather than any
/// invalid-command error: a caller reading the message has to be able to tell
/// that the scanner declined to place a pending body.
pub(super) fn assert_both_traversals_reject_heredoc_boundary(recipe: Recipe<'_>, rule: &str) {
    let bindings = CommandBindings::new(&[], &[], RecipeShell::Posix);
    for result in [
        substitute(recipe.0, &bindings),
        substitute_script(recipe.0, &bindings),
    ] {
        let error = result.expect_err(rule);
        assert!(
            matches!(error, IrGenError::InvalidCommand { .. }),
            "{rule}: expected an invalid-command error, got {error:?}"
        );
        assert!(
            error.to_string().contains("unsupported heredoc boundary"),
            "{rule}: the diagnostic must name the rejected boundary: {error}"
        );
    }
}
