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
            template,
            "marker-free ANSI-C text changed via {lowering:?}: {template:?}"
        );
    }
}

/// Lower a marker after an ANSI-C string using the unquoted encoding.
#[rstest]
#[case::command(Lowering::Command)]
#[case::script(Lowering::Script)]
fn marker_after_ansi_c_region_uses_unquoted_encoding(#[case] lowering: Lowering) {
    // The path encodes differently in every quote context, so the expected
    // text below fails if the traversal selects any encoding but the
    // unquoted one.
    let bindings = context_bindings(RecipeShell::Bash, "in put$x");
    let template = format!("echo $'a\\tb' {INS_TOKEN}");
    assert_eq!(
        lowering
            .lower(&template, &bindings)
            .expect("a marker after the closing quote is outside ANSI-C"),
        "echo $'a\\tb' in' put$x'",
        "post-region marker used the wrong encoding via {lowering:?}: {template:?}"
    );
}

#[rstest]
#[case::escaped_dollar("echo \\$'{marker}'", "a'b", "echo \\$'a'\"'\"'b'")]
#[case::pid_expansion_dollar("echo $$'{marker}'", "a'b", "echo $$'a'\"'\"'b'")]
#[case::dollar_inside_double_quotes("echo \"$'{marker}'\"", "a$b", "echo \"$'a\\$b'\"")]
#[case::dollar_inside_single_quotes("echo '$''{marker}'", "a'b", "echo '$''a'\"'\"'b'")]
fn ansi_c_lookbehind_keeps_the_surrounding_quote_context(
    #[case] template: &str,
    #[case] path: &str,
    #[case] expected: &str,
    #[values(Lowering::Command, Lowering::Script)] lowering: Lowering,
) {
    let bindings = context_bindings(RecipeShell::Bash, path);
    let expanded = template.replace("{marker}", INS_TOKEN);
    assert_eq!(
        lowering
            .lower(&expanded, &bindings)
            .expect("a marker outside ANSI-C must still lower"),
        expected,
        "wrong quote context via {lowering:?}: {expanded:?}"
    );
}

/// Do not carry ANSI-C look-behind state across a lowered placeholder.
///
/// Lowering replaces the rest of the token without scanning it, so a dollar
/// before the placeholder and one after it must not pair as `$$`. If they did,
/// the `$'` opening an ANSI-C string after the placeholder would be read as a
/// PID expansion followed by ordinary quotes, and its marker would take the
/// unsafe single-quoted encoding.
///
/// This is a contract test, not a mutation detector for the reset that runs
/// after lowering: the internal token begins with `_`, and that ordinary
/// character already clears both the pending opener and the `$$` pair before
/// the reset sees them. The reset stays as a guard that does not depend on how
/// a placeholder is spelled.
#[rstest]
#[case::posix(RecipeShell::Posix)]
#[case::bash(RecipeShell::Bash)]
fn placeholder_separates_ansi_c_lookbehind(#[case] shell: RecipeShell) {
    let bindings = ansi_c_leading_bindings(shell);
    let template = format!("echo ${INS_TOKEN}$'{OUTS_TOKEN}'");
    let error = interpolate_script_with_bindings(&template, &bindings)
        .expect_err("a placeholder must not let `$$` parity hide the ANSI-C opener");
    assert!(
        matches!(
            error,
            IrGenError::InvalidCommand { ref command, .. } if *command == template
        ),
        "rejection must report the original template, got: {error:?}"
    );
}

/// Reject a marker inside ANSI-C opened after an empty command substitution.
///
/// `$$()` leaves an unpaired dollar at the first `$`: the second `$` starts the
/// substitution and never reaches quote tracking, and the closer returns
/// through the delimiter path. Only the two resets taken at the region's
/// boundaries stop that dollar pairing with the one in `$'` after it, so this
/// case separates their removal as a pair. Either reset alone still clears the
/// state, which is why this case claims no per-site detection.
#[rstest]
#[case::posix_command(RecipeShell::Posix, Lowering::Command)]
#[case::posix_script(RecipeShell::Posix, Lowering::Script)]
#[case::bash_command(RecipeShell::Bash, Lowering::Command)]
#[case::bash_script(RecipeShell::Bash, Lowering::Script)]
fn command_substitution_region_resets_ansi_c_lookbehind(
    #[case] shell: RecipeShell,
    #[case] lowering: Lowering,
) {
    let bindings = ansi_c_leading_bindings(shell);
    for marker in [INS_TOKEN, OUTS_TOKEN] {
        let template = format!("echo $$()$'{marker}'");
        let error = lowering
            .lower(&template, &bindings)
            .expect_err("a marker inside ANSI-C after `$$()` must fail closed");
        assert!(
            matches!(
                error,
                IrGenError::InvalidCommand { ref command, .. } if *command == template
            ),
            "rejection must report the original template, got: {error:?}"
        );
    }
}
