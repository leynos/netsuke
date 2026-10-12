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

/// Verify a PowerShell escape cannot hide a recipe marker from validation.
#[test]
fn power_shell_rejects_markers_after_backticks() {
    let bindings = CommandBindings::new(&[], &[Utf8PathBuf::from("out")], RecipeShell::PowerShell);
    let error =
        interpolate_command_with_bindings(&format!("Write-Output `{OUTS_TOKEN}"), &bindings)
            .expect_err("PowerShell backticks before markers must not bypass validation");
    assert!(matches!(error, IrGenError::InvalidCommand { .. }));
}

/// Prepare bindings with a Windows-valid path that can expose source injection.
fn script_bindings() -> CommandBindings {
    CommandBindings::new(
        &[Utf8PathBuf::from("; New-Item pwned; Write-Output x")],
        &[Utf8PathBuf::from("output file.txt")],
        RecipeShell::PowerShell,
    )
}

/// Reject every PowerShell script marker in contexts that need shell parsing.
#[rstest]
#[case::single_quoted(format!("Write-Output '{INS_TOKEN}' > {OUTS_TOKEN}"))]
#[case::double_quoted(format!("Write-Output \"{INS_TOKEN}\" > {OUTS_TOKEN}"))]
#[case::command_substitution(format!("Write-Output $(Write-Output {INS_TOKEN}) > {OUTS_TOKEN}"))]
#[case::escaped(format!("Write-Output `{INS_TOKEN} > {OUTS_TOKEN}"))]
#[case::single_quoted_here_string(format!("@'\n{INS_TOKEN}\n'@ > {OUTS_TOKEN}"))]
#[case::double_quoted_here_string(format!("@\"\n{INS_TOKEN}\n\"@ > {OUTS_TOKEN}"))]
fn power_shell_scripts_reject_protected_markers(#[case] template: String) {
    let result = interpolate_script_with_bindings(&template, &script_bindings());
    let error = result.expect_err("PowerShell scripts must reject protected markers");
    let IrGenError::InvalidCommand { command, .. } = error else {
        panic!("expected InvalidCommand for {template:?}, got {error:?}");
    };

    assert_eq!(
        command, template,
        "the diagnostic should retain the template"
    );
}

/// Accept unquoted markers and encode the semicolon path as one PowerShell word.
#[test]
fn power_shell_scripts_quote_unquoted_paths_in_multiline_text() {
    let template = format!("Copy-Item {INS_TOKEN} {OUTS_TOKEN}\nWrite-Output done");
    let actual = interpolate_script_with_bindings(&template, &script_bindings())
        .expect("unquoted PowerShell script markers should interpolate");

    assert_eq!(
        actual,
        "Copy-Item '; New-Item pwned; Write-Output x' 'output file.txt'\nWrite-Output done"
    );
}
