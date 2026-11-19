//! Regression tests for markers inside Bash dollar-single-quoted strings.
//!
//! Bash's ANSI-C quoting (`$'…'`) gives a backslash control over the character
//! after it, so an apostrophe preceded by a backslash does not close the
//! string. The ordinary single-quoted encoder rewrites every apostrophe as
//! `'"'"'`; inside `$'…'` a path that ends a backslash immediately before that
//! rewrite closes the ANSI-C string early and leaves the remainder of the path
//! as executable shell syntax (CWE-78, finding
//! `b247e1c0-c546-4ac4-a8e1-ddb892fc9cee`). The traversal cannot know whether
//! the child interpreter reads `$'…'` as ANSI-C quoting, so both traversals
//! reject markers in that region instead of encoding them. This is separate
//! from the active-escape rejection for #629, which covers a marker directly
//! after a backslash.

use super::*;

use camino::Utf8PathBuf;
use rstest::rstest;

/// Path text that closes an ANSI-C string when single-quote encoded.
const ANSI_C_LEADING_PATH: &str = "\\'; touch injected; #";

/// Select the traversal whose ANSI-C handling a case exercises.
#[derive(Clone, Copy, Debug)]
enum Lowering {
    /// The `command:` traversal.
    Command,
    /// The POSIX `script:` traversal.
    Script,
}

impl Lowering {
    /// Lower one template through this traversal.
    fn lower(self, template: &str, bindings: &CommandBindings) -> Result<String, IrGenError> {
        match self {
            Self::Command => interpolate_command_with_bindings(template, bindings),
            Self::Script => interpolate_script_with_bindings(template, bindings),
        }
    }
}

/// Build bindings whose input and output paths both break out of ANSI-C quoting.
fn ansi_c_leading_bindings(shell: RecipeShell) -> CommandBindings {
    CommandBindings::new(
        &[Utf8PathBuf::from(ANSI_C_LEADING_PATH)],
        &[Utf8PathBuf::from(ANSI_C_LEADING_PATH)],
        shell,
    )
}

/// Reject a marker reached while an ANSI-C string is open.
///
/// The table pins every way the opener can survive into the marker: a direct
/// opener, an escaped apostrophe that keeps the region open, a backslash-newline
/// continuation between the dollar and the quote, and a third dollar after a
/// `$$` pair. All four are ANSI-C strings to Bash, and none can be encoded with
/// the ordinary single-quoted binding.
#[rstest]
#[case::posix_command(RecipeShell::Posix, Lowering::Command)]
#[case::posix_script(RecipeShell::Posix, Lowering::Script)]
#[case::bash_command(RecipeShell::Bash, Lowering::Command)]
#[case::bash_script(RecipeShell::Bash, Lowering::Script)]
fn markers_inside_ansi_c_strings_are_rejected(
    #[case] shell: RecipeShell,
    #[case] lowering: Lowering,
) {
    let bindings = ansi_c_leading_bindings(shell);
    for marker in [INS_TOKEN, OUTS_TOKEN] {
        for template in [
            format!("echo $'{marker}'"),
            format!("echo $'a\\'{marker}'"),
            format!("echo $\\\n'{marker}'"),
            format!("echo $$$'{marker}'"),
        ] {
            let error = lowering
                .lower(&template, &bindings)
                .expect_err("a marker inside ANSI-C quoting must fail closed");
            assert!(
                matches!(
                    error,
                    IrGenError::InvalidCommand { ref command, .. } if *command == template
                ),
                "rejection must report the original template, got: {error:?}"
            );
        }
    }
}

/// Build bindings whose input path uses `path` in every quote context.
fn context_bindings(shell: RecipeShell, path: &str) -> CommandBindings {
    CommandBindings::new(
        &[Utf8PathBuf::from(path)],
        &[Utf8PathBuf::from(path)],
        shell,
    )
}

/// Leave ANSI-C text that contains no marker byte-for-byte intact.
#[rstest]
#[case::posix_command(RecipeShell::Posix, Lowering::Command)]
#[case::posix_script(RecipeShell::Posix, Lowering::Script)]
#[case::bash_command(RecipeShell::Bash, Lowering::Command)]
#[case::bash_script(RecipeShell::Bash, Lowering::Script)]
fn marker_free_ansi_c_strings_are_preserved(
    #[case] shell: RecipeShell,
    #[case] lowering: Lowering,
) {
    let bindings = context_bindings(shell, "input");
    for template in ["echo $'a\\tb'", "printf '%s' $'line\\n'"] {
        assert_eq!(
            lowering
                .lower(template, &bindings)
                .expect("marker-free ANSI-C text must lower unchanged"),
            template
        );
    }
}

/// Lower a marker after an ANSI-C string using the unquoted encoding.
#[rstest]
#[case::command(Lowering::Command)]
#[case::script(Lowering::Script)]
fn marker_after_ansi_c_region_uses_unquoted_encoding(#[case] lowering: Lowering) {
    let bindings = context_bindings(RecipeShell::Bash, "input");
    let template = format!("echo $'a\\tb' {INS_TOKEN}");
    assert_eq!(
        lowering
            .lower(&template, &bindings)
            .expect("a marker after the closing quote is outside ANSI-C"),
        "echo $'a\\tb' input"
    );
}

/// Keep an escaped apostrophe from closing the ANSI-C string early.
#[test]
fn escaped_apostrophe_does_not_close_ansi_c() {
    let bindings = context_bindings(RecipeShell::Bash, "input");
    let template = format!("echo $'it\\'s' {INS_TOKEN}");
    assert_eq!(
        interpolate_script_with_bindings(&template, &bindings)
            .expect("the escaped apostrophe must not close the ANSI-C string"),
        "echo $'it\\'s' input"
    );
}

/// Treat a dollar escaped before a quote as an ordinary single-quoted string.
#[rstest]
#[case::command(Lowering::Command)]
#[case::script(Lowering::Script)]
fn escaped_dollar_before_quote_uses_single_quoted_encoding(#[case] lowering: Lowering) {
    let bindings = context_bindings(RecipeShell::Bash, "a'b");
    let template = format!("echo \\$'{INS_TOKEN}'");
    assert_eq!(
        lowering
            .lower(&template, &bindings)
            .expect("an escaped dollar cannot open ANSI-C"),
        "echo \\$'a'\"'\"'b'"
    );
}

/// Treat the second dollar of `$$` as the PID expansion, not an opener.
#[rstest]
#[case::command(Lowering::Command)]
#[case::script(Lowering::Script)]
fn double_dollar_before_quote_uses_single_quoted_encoding(#[case] lowering: Lowering) {
    let bindings = context_bindings(RecipeShell::Bash, "a'b");
    let template = format!("echo $$'{INS_TOKEN}'");
    assert_eq!(
        lowering
            .lower(&template, &bindings)
            .expect("the second dollar of a PID expansion cannot open ANSI-C"),
        "echo $$'a'\"'\"'b'"
    );
}

/// Treat a dollar and quote inside double quotes as literal text.
#[rstest]
#[case::command(Lowering::Command)]
#[case::script(Lowering::Script)]
fn dollar_and_quote_inside_double_quotes_use_double_encoding(#[case] lowering: Lowering) {
    let bindings = context_bindings(RecipeShell::Bash, "a$b");
    let template = format!("echo \"$'{INS_TOKEN}'\"");
    assert_eq!(
        lowering
            .lower(&template, &bindings)
            .expect("ANSI-C is not opened inside double quotes"),
        "echo \"$'a\\$b'\""
    );
}

/// Leave a `$'` sequence inside ordinary single quotes as literal text.
#[rstest]
#[case::command(Lowering::Command)]
#[case::script(Lowering::Script)]
fn dollar_quote_inside_ordinary_single_quotes_uses_single_encoding(#[case] lowering: Lowering) {
    let bindings = context_bindings(RecipeShell::Bash, "a'b");
    let template = format!("echo '$''{INS_TOKEN}'");
    assert_eq!(
        lowering
            .lower(&template, &bindings)
            .expect("a dollar inside ordinary single quotes cannot open ANSI-C"),
        "echo '$''a'\"'\"'b'"
    );
}
