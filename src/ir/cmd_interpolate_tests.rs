//! Unit tests for command interpolation and shell-specific validation.

use super::*;

use camino::Utf8PathBuf;

/// Verify POSIX interpolation rejects commands with unbalanced backticks.
#[test]
fn interpolate_command_rejects_unbalanced_backticks() {
    let path = Utf8PathBuf::from("a");
    let bindings = CommandBindings::new(
        std::slice::from_ref(&path),
        std::slice::from_ref(&path),
        RecipeShell::Posix,
    );
    let err = interpolate_command_with_bindings("echo `", &bindings)
        .expect_err("command should be rejected");
    match err {
        IrGenError::InvalidCommand { command, .. } => {
            assert_eq!(command, "echo `");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

/// Verify POSIX interpolation replaces short input and output placeholders.
#[test]
fn interpolate_command_replaces_placeholders() {
    let ins = vec![Utf8PathBuf::from("in"), Utf8PathBuf::from("aux")];
    let outs = vec![Utf8PathBuf::from("out")];
    let command = interpolate_command_with_shell("cp $in $out", &ins, &outs, RecipeShell::Posix)
        .expect("command");
    assert_eq!(command, "cp in aux out");
}

/// Verify POSIX interpolation rejects short placeholders inside backticks.
#[test]
fn interpolate_command_rejects_short_placeholders_in_backticks() {
    let ins = vec![Utf8PathBuf::from("src")];
    let outs = vec![Utf8PathBuf::from("out")];
    let error = interpolate_command_with_shell(
        "echo `cat $in` && echo $out",
        &ins,
        &outs,
        RecipeShell::Posix,
    )
    .expect_err("placeholders inside backticks should be rejected");
    assert!(matches!(error, IrGenError::InvalidCommand { .. }));
}

/// Verify POSIX interpolation rejects template placeholders inside backticks.
#[test]
fn interpolate_command_rejects_template_placeholders_in_backticks() {
    let error = interpolate_command_with_shell(
        &format!("echo `{INS_TOKEN}` $out"),
        &[],
        &[Utf8PathBuf::from("out")],
        RecipeShell::Posix,
    )
    .expect_err("template placeholders inside backticks should be rejected");
    assert!(matches!(error, IrGenError::InvalidCommand { .. }));
}

/// Verify POSIX interpolation replaces template input and output placeholders.
#[test]
fn interpolate_command_replaces_template_placeholders() {
    let command = interpolate_command_with_shell(
        &format!("{INS_TOKEN} $out {OUTS_TOKEN}"),
        &[Utf8PathBuf::from("in")],
        &[Utf8PathBuf::from("out")],
        RecipeShell::Posix,
    )
    .expect("command");
    assert_eq!(command, "in out out");
}

/// Verify that PowerShell interpolation doubles apostrophes in single-quoted literals.
#[test]
fn power_shell_bindings_quote_apostrophes_as_single_literals() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("source's file")],
        &[Utf8PathBuf::from("output's file")],
        RecipeShell::PowerShell,
    );
    let command = interpolate_command_with_bindings("Copy-Item $in $out", &bindings)
        .expect("PowerShell-safe placeholders should interpolate");
    assert_eq!(command, "Copy-Item 'source''s file' 'output''s file'");
}

#[test]
fn power_shell_bindings_preserve_literal_backticks_in_paths() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("source`file")],
        &[Utf8PathBuf::from("output`file")],
        RecipeShell::PowerShell,
    );
    let command = interpolate_command_with_bindings("Copy-Item $in $out", &bindings)
        .expect("PowerShell single-quoted paths should preserve literal backticks");
    assert_eq!(command, "Copy-Item 'source`file' 'output`file'");
}
/// Verify that PowerShell-specific escaped double quotes bypass POSIX syntax checks.
#[test]
fn power_shell_bindings_allow_escaped_double_quotes() {
    let bindings = CommandBindings::new(&[], &[], RecipeShell::PowerShell);
    let command = interpolate_command_with_bindings(r#"Write-Output "a`"b""#, &bindings)
        .expect("PowerShell escaped double quotes should interpolate");
    assert_eq!(command, r#"Write-Output "a`"b""#);
}
