//! Regression tests for POSIX markers that directly follow a shell escape.
//!
//! A backslash retained immediately before a marker decides the meaning of the
//! first character of the substituted text. Between double quotes the path
//! encoding writes `\"` for a leading double quote, so a retained backslash
//! turns the pair into `\\"`: the escape is consumed and the quote terminates
//! the string, leaving the rest of the path as executable shell syntax. The
//! traversals therefore reject these sites instead of encoding them, while
//! continuing to accept a backslash that forms an escaped pair.

use super::*;

use camino::Utf8PathBuf;
use rstest::rstest;

/// Path text whose double-quoted encoding begins with an escape sequence.
const QUOTE_LEADING_PATH: &str = "\"; touch injected; #";

/// Select the traversal whose escape handling a case exercises.
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

/// Build bindings whose input and output paths both begin with a double quote.
fn quote_leading_bindings(shell: RecipeShell) -> CommandBindings {
    CommandBindings::new(
        &[Utf8PathBuf::from(QUOTE_LEADING_PATH)],
        &[Utf8PathBuf::from(QUOTE_LEADING_PATH)],
        shell,
    )
}

/// Reject a marker that follows an unpaired backslash outside single quotes.
///
/// One backslash is the active-escape site; three leave the third backslash
/// unpaired after the leading pair is consumed. Both must fail closed rather
/// than lower a binding whose encoding the retained backslash can cancel.
#[rstest]
#[case::posix_command(RecipeShell::Posix, Lowering::Command)]
#[case::posix_script(RecipeShell::Posix, Lowering::Script)]
#[case::bash_command(RecipeShell::Bash, Lowering::Command)]
#[case::bash_script(RecipeShell::Bash, Lowering::Script)]
fn markers_after_an_active_backslash_are_rejected(
    #[case] shell: RecipeShell,
    #[case] lowering: Lowering,
) {
    let bindings = quote_leading_bindings(shell);
    for escapes in ["\\", "\\\\\\"] {
        for marker in [INS_TOKEN, OUTS_TOKEN] {
            for template in [
                format!("echo {escapes}{marker}"),
                format!("echo \"{escapes}{marker}\""),
            ] {
                let error = lowering
                    .lower(&template, &bindings)
                    .expect_err("a marker after an active POSIX escape must fail closed");
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
}

/// Keep accepting escapes that pair with the backslash after them.
///
/// Two backslashes are consumed as one escaped backslash before the marker is
/// reached, so the ordinary context encoding still applies and the path text
/// stays a single argument.
#[rstest]
#[case::quote_leading(
    QUOTE_LEADING_PATH,
    "echo \\\\'\"; touch injected; #'",
    "echo \"\\\\\\\"; touch injected; #\""
)]
#[case::dollar_leading("$x", "echo \\\\'$x'", "echo \"\\\\\\$x\"")]
#[case::backslash_leading("\\x", "echo \\\\'\\x'", "echo \"\\\\\\\\x\"")]
fn paired_backslashes_before_markers_are_encoded(
    #[case] path: &str,
    #[case] expected_unquoted: &str,
    #[case] expected_double_quoted: &str,
) {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from(path)],
        &[Utf8PathBuf::from(path)],
        RecipeShell::Posix,
    );
    for (template, expected) in [
        (
            format!("echo \\\\{INS_TOKEN}"),
            expected_unquoted.to_owned(),
        ),
        (
            format!("echo \"\\\\{INS_TOKEN}\""),
            expected_double_quoted.to_owned(),
        ),
    ] {
        assert_eq!(
            interpolate_command_with_bindings(&template, &bindings)
                .expect("a paired backslash must not reject the marker"),
            expected
        );
        assert_eq!(
            interpolate_script_with_bindings(&template, &bindings)
                .expect("a paired backslash must not reject a script marker"),
            expected
        );
    }
}

/// Keep a single-quoted backslash literal because it escapes nothing there.
#[test]
fn single_quoted_backslashes_leave_markers_eligible() {
    let bindings = quote_leading_bindings(RecipeShell::Posix);
    let template = format!("echo '\\{INS_TOKEN}'");
    let expected = format!("echo '\\{QUOTE_LEADING_PATH}'");
    assert_eq!(
        interpolate_command_with_bindings(&template, &bindings)
            .expect("a single-quoted backslash must not reject the marker"),
        expected
    );
    assert_eq!(
        interpolate_script_with_bindings(&template, &bindings)
            .expect("a single-quoted backslash must not reject a script marker"),
        expected
    );
}

/// Keep a backslash-newline continuation away from the escape check.
///
/// The backslash escapes the newline rather than the marker, so the marker is
/// still reached with an ordinary context encoding.
#[test]
fn backslash_newline_continuations_leave_markers_eligible() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("input")],
        &[Utf8PathBuf::from("output")],
        RecipeShell::Posix,
    );
    let template = format!("echo \\\n{INS_TOKEN}");
    let expected = "echo \\\ninput";
    assert_eq!(
        interpolate_script_with_bindings(&template, &bindings)
            .expect("a continued line must not reject the marker"),
        expected
    );
}

/// Leave comments, heredoc bodies, and shell variables byte-for-byte intact.
#[test]
fn inert_text_and_shell_variables_ignore_the_escape_check() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("input")],
        &[Utf8PathBuf::from("output")],
        RecipeShell::Posix,
    );
    for template in [
        format!("# \\{INS_TOKEN}\necho hi"),
        "echo $in $out $ins $outs \\$PATH".to_owned(),
    ] {
        assert_eq!(
            interpolate_script_with_bindings(&template, &bindings)
                .expect("inert text must not be treated as a marker site"),
            template
        );
    }
    let heredoc = format!("cat <<EOF\n\\{OUTS_TOKEN}\nEOF\necho {INS_TOKEN}");
    assert_eq!(
        interpolate_script_with_bindings(&heredoc, &bindings)
            .expect("heredoc bodies must remain inert"),
        "cat <<EOF\n\\__NETSUKE_OUTS_PLACEHOLDER__\nEOF\necho input"
    );
}
