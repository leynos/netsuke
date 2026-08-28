//! Unit tests for command interpolation and shell-specific validation.

use super::*;

use camino::Utf8PathBuf;

#[test]
fn interpolate_command_rejects_unbalanced_backticks() {
    let path = Utf8PathBuf::from("a");
    let err = interpolate_command(
        "echo `",
        std::slice::from_ref(&path),
        std::slice::from_ref(&path),
    )
    .expect_err("command should be rejected");
    match err {
        IrGenError::InvalidCommand { command, .. } => {
            assert_eq!(command, "echo `");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn interpolate_command_replaces_placeholders() {
    let ins = vec![Utf8PathBuf::from("in"), Utf8PathBuf::from("aux")];
    let outs = vec![Utf8PathBuf::from("out")];
    let command = interpolate_command("cp $in $out", &ins, &outs).expect("command");
    assert_eq!(command, "cp in aux out");
}

#[test]
fn interpolate_command_rejects_short_placeholders_in_backticks() {
    let ins = vec![Utf8PathBuf::from("src")];
    let outs = vec![Utf8PathBuf::from("out")];
    let error = interpolate_command("echo `cat $in` && echo $out", &ins, &outs)
        .expect_err("placeholders inside backticks should be rejected");
    assert!(matches!(error, IrGenError::InvalidCommand { .. }));
}

#[test]
fn interpolate_command_rejects_template_placeholders_in_backticks() {
    let error = interpolate_command(
        &format!("echo `{INS_TOKEN}` $out"),
        &[],
        &[Utf8PathBuf::from("out")],
    )
    .expect_err("template placeholders inside backticks should be rejected");
    assert!(matches!(error, IrGenError::InvalidCommand { .. }));
}

#[test]
fn interpolate_command_replaces_template_placeholders() {
    let command = interpolate_command(
        &format!("{INS_TOKEN} $out {OUTS_TOKEN}"),
        &[Utf8PathBuf::from("in")],
        &[Utf8PathBuf::from("out")],
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

/// Verify that PowerShell-specific escaped double quotes bypass POSIX syntax checks.
#[test]
fn power_shell_bindings_allow_escaped_double_quotes() {
    let bindings = CommandBindings::new(&[], &[], RecipeShell::PowerShell);
    let command = interpolate_command_with_bindings(r#"Write-Output "a`"b""#, &bindings)
        .expect("PowerShell escaped double quotes should interpolate");
    assert_eq!(command, r#"Write-Output "a`"b""#);
}
