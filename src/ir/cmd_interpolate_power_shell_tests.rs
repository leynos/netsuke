//! Test PowerShell recipe-marker protection contexts separately from POSIX cases.

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
