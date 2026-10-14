//! Test PowerShell scanner boundaries involving Unicode whitespace and quotes.

use super::*;

use camino::Utf8PathBuf;
use rstest::rstest;

#[rstest]
#[case::comment_after_nonbreaking_space(format!(
    "Write-Output x\u{00a0}# '\nWrite-Output '{INS_TOKEN}'"
))]
#[case::stop_parsing_after_unicode_spaces(format!(
    "cmd /c\u{00a0}--%\u{3000}Write-Output {INS_TOKEN}"
))]
fn commands_and_scripts_reject_markers_after_unicode_boundaries(#[case] template: String) {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("; New-Item pwned; Write-Output x")],
        &[Utf8PathBuf::from("output file.txt")],
        RecipeShell::PowerShell,
    );

    let script_error = interpolate_script_with_bindings(&template, &bindings)
        .expect_err("PowerShell script markers after Unicode boundaries must fail closed");
    assert!(matches!(
        script_error,
        IrGenError::InvalidCommand { command, .. } if command == template
    ));

    let command_error = interpolate_command_with_bindings(&template, &bindings)
        .expect_err("PowerShell command markers after Unicode boundaries must fail closed");
    assert!(matches!(
        command_error,
        IrGenError::InvalidCommand { command, .. } if command == template
    ));
}

#[test]
fn power_shell_commands_reject_markers_after_typographic_here_string_footers() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("; New-Item pwned; Write-Output x")],
        &[Utf8PathBuf::from("output file.txt")],
        RecipeShell::PowerShell,
    );
    let template = format!("@'\n’@ + \"\n'@\nWrite-Output {INS_TOKEN}\n\"");
    let error = interpolate_command_with_bindings(&template, &bindings)
        .expect_err("a marker after an unsupported here-string footer must fail closed");

    assert!(matches!(
        error,
        IrGenError::InvalidCommand { command, .. } if command == template
    ));
}
