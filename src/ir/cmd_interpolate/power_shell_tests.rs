//! Test PowerShell recipe-marker protection contexts for commands and scripts.

use super::*;

use camino::Utf8PathBuf;
use rstest::rstest;

/// Verify PowerShell rejects markers in quote and command-substitution regions.
#[rstest]
#[case::single_quoted(format!("Write-Output '{OUTS_TOKEN}'"))]
#[case::double_quoted(format!("Write-Output \"{OUTS_TOKEN}\""))]
#[case::command_substitution(format!("Write-Output $(Write-Output {OUTS_TOKEN})"))]
fn power_shell_rejects_markers_without_a_context_safe_encoder(#[case] template: String) {
    let bindings = CommandBindings::new(&[], &[Utf8PathBuf::from("out")], RecipeShell::PowerShell);
    let result = interpolate_command_with_bindings(&template, &bindings);

    assert!(
        matches!(&result, Err(IrGenError::InvalidCommand { .. })),
        "PowerShell template {template:?} should reject an unsafe marker, got {result:?}"
    );
}

/// Reject command markers when a mid-token hash may hide quote syntax in a comment.
#[rstest]
#[case::input(format!("$value#'\nWrite-Output '{INS_TOKEN}'"))]
#[case::output(format!("$value#'\nSet-Content output '{OUTS_TOKEN}'"))]
fn power_shell_commands_reject_markers_after_ambiguous_comment_quotes(#[case] template: String) {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("; New-Item pwned; Write-Output x")],
        &[Utf8PathBuf::from("out")],
        RecipeShell::PowerShell,
    );
    let error = interpolate_command_with_bindings(&template, &bindings)
        .expect_err("ambiguous comment quotes must not expose marker sites");

    assert!(matches!(error, IrGenError::InvalidCommand { command, .. } if command == template));
}

/// Reject a command marker when a postfix operator may start a comment.
#[test]
fn power_shell_commands_reject_markers_after_increment_comment_quotes() {
    let template = format!("$i++#'\nWrite-Output '{OUTS_TOKEN}'");
    let bindings = CommandBindings::new(&[], &[Utf8PathBuf::from("out")], RecipeShell::PowerShell);
    let error = interpolate_command_with_bindings(&template, &bindings)
        .expect_err("postfix-expression comments must not expose quoted marker sites");

    assert!(matches!(error, IrGenError::InvalidCommand { command, .. } if command == template));
}

/// Verify a PowerShell escape cannot hide a recipe marker from validation.
#[test]
fn power_shell_rejects_markers_after_backticks() {
    let bindings = CommandBindings::new(&[], &[Utf8PathBuf::from("out")], RecipeShell::PowerShell);
    let error =
        interpolate_command_with_bindings(&format!("Write-Output `{OUTS_TOKEN}"), &bindings)
            .expect_err("PowerShell backticks before markers must not bypass validation");
    assert!(matches!(error, IrGenError::InvalidCommand { .. }));
}

/// Reject a command marker in PowerShell's raw native-command argument region.
#[test]
fn power_shell_commands_reject_markers_after_stop_parsing_token() {
    let template = format!("cmd /c --% type {INS_TOKEN}");
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("; New-Item pwned; Write-Output x")],
        &[],
        RecipeShell::PowerShell,
    );
    let error = interpolate_command_with_bindings(&template, &bindings)
        .expect_err("raw native-command arguments cannot contain path markers");

    assert!(matches!(error, IrGenError::InvalidCommand { command, .. } if command == template));
}

/// Reject command markers after stop-parsing quote syntax the scanner cannot model.
#[rstest]
#[case::single_quote(format!("cmd /c --% 'prefix | text' | Write-Output {OUTS_TOKEN}"))]
#[case::backtick_before_double_quote(format!(
    "cmd /c --% \"prefix`\" still \" | Write-Output {OUTS_TOKEN}"
))]
fn power_shell_commands_reject_markers_after_ambiguous_stop_parsing_syntax(
    #[case] template: String,
) {
    let bindings = CommandBindings::new(&[], &[Utf8PathBuf::from("out")], RecipeShell::PowerShell);
    let error = interpolate_command_with_bindings(&template, &bindings)
        .expect_err("ambiguous stop-parsing syntax must fail closed for commands");

    assert!(matches!(error, IrGenError::InvalidCommand { command, .. } if command == template));
}

/// Preserve an argument hash after punctuation that remains part of its token.
#[test]
fn power_shell_commands_preserve_hash_after_argument_asterisk() {
    let template = format!("Write-Output foo*#bar \"quoted\" {OUTS_TOKEN}");
    let bindings = CommandBindings::new(&[], &[Utf8PathBuf::from("out")], RecipeShell::PowerShell);
    let actual = interpolate_command_with_bindings(&template, &bindings)
        .expect("an asterisk inside an argument does not start a comment");

    assert_eq!(actual, "Write-Output foo*#bar \"quoted\" 'out'");
}

/// Preserve hashes after argument characters that do not start comments.
#[rstest]
#[case::assignment("a=#b")]
#[case::closing_bracket("a]#b")]
fn power_shell_commands_preserve_hashes_inside_argument_tokens(#[case] argument: &str) {
    let template = format!("Write-Output {argument} {OUTS_TOKEN}");
    let bindings = CommandBindings::new(&[], &[Utf8PathBuf::from("out")], RecipeShell::PowerShell);
    let actual = interpolate_command_with_bindings(&template, &bindings)
        .expect("hashes after assignment and bracket text remain argument content");

    assert_eq!(actual, format!("Write-Output {argument} 'out'"));
}
