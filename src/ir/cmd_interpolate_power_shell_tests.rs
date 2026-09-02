//! Test PowerShell recipe-marker protection contexts separately from POSIX cases.

use super::*;

use camino::Utf8PathBuf;

/// Verify PowerShell rejects markers in quote and command-substitution regions.
#[test]
fn power_shell_rejects_markers_without_a_context_safe_encoder() {
    let bindings = CommandBindings::new(&[], &[Utf8PathBuf::from("out")], RecipeShell::PowerShell);
    for template in [
        format!("Write-Output '{OUTS_TOKEN}'"),
        format!("Write-Output \"{OUTS_TOKEN}\""),
        format!("Write-Output $(Write-Output {OUTS_TOKEN})"),
    ] {
        assert!(matches!(
            interpolate_command_with_bindings(&template, &bindings),
            Err(IrGenError::InvalidCommand { .. })
        ));
    }
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
