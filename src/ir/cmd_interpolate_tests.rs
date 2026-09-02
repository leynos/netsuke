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

/// Verify POSIX interpolation replaces manifest-owned input and output markers.
#[test]
fn interpolate_command_replaces_placeholders() {
    let ins = vec![Utf8PathBuf::from("in"), Utf8PathBuf::from("aux")];
    let outs = vec![Utf8PathBuf::from("out")];
    let command = interpolate_command_with_shell(
        &format!("cp {INS_TOKEN} {OUTS_TOKEN}"),
        &ins,
        &outs,
        RecipeShell::Posix,
    )
    .expect("command");
    assert_eq!(command, "cp in aux out");
}

/// Verify POSIX interpolation preserves dollar-prefixed shell variables.
#[test]
fn interpolate_command_preserves_dollar_prefixed_shell_variables() {
    let ins = vec![Utf8PathBuf::from("src")];
    let outs = vec![Utf8PathBuf::from("out")];
    let error =
        interpolate_command_with_shell("echo $in $out $ins $outs", &ins, &outs, RecipeShell::Posix)
            .expect("literal shell variables must remain valid");
    assert_eq!(error, "echo $in $out $ins $outs");
}
/// Verify POSIX interpolation rejects template placeholders inside backticks.
#[test]
fn interpolate_command_rejects_template_placeholders_in_backticks() {
    let error = interpolate_command_with_shell(
        &format!("echo `{INS_TOKEN}` {OUTS_TOKEN}"),
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
        &format!("{INS_TOKEN} {OUTS_TOKEN}"),
        &[Utf8PathBuf::from("in")],
        &[Utf8PathBuf::from("out")],
        RecipeShell::Posix,
    )
    .expect("command");
    assert_eq!(command, "in out");
}

/// Verify that PowerShell interpolation doubles apostrophes in marker replacements.
#[test]
fn power_shell_bindings_quote_apostrophes_as_single_literals() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("source's file")],
        &[Utf8PathBuf::from("output's file")],
        RecipeShell::PowerShell,
    );
    let command = interpolate_command_with_bindings(
        &format!("Copy-Item {INS_TOKEN} {OUTS_TOKEN}"),
        &bindings,
    )
    .expect("PowerShell-safe placeholders should interpolate");
    assert_eq!(command, "Copy-Item 'source''s file' 'output''s file'");
}

/// Verify POSIX markers are encoded safely in unquoted and quoted command text.
#[test]
fn interpolate_command_encodes_markers_for_quote_contexts() {
    for (marker, expected_path, unquoted, double_quoted) in [
        (INS_TOKEN, "input", "input", "input"),
        (
            OUTS_TOKEN,
            "x\";id;echo\"y",
            "x'\";id;echo\"y'",
            "x\\\";id;echo\\\"y",
        ),
    ] {
        let cases = [
            (format!("echo {marker}"), format!("echo {unquoted}")),
            (
                format!("echo '{marker}'"),
                format!("echo '{expected_path}'"),
            ),
            (
                format!("echo \"{marker}\""),
                format!("echo \"{double_quoted}\""),
            ),
        ];
        for (template, expected) in cases {
            let command = interpolate_command_with_shell(
                &template,
                &[Utf8PathBuf::from("input")],
                &[Utf8PathBuf::from("x\";id;echo\"y")],
                RecipeShell::Posix,
            )
            .expect("context-safe command should interpolate");
            assert_eq!(command, expected);
        }
    }
}

/// Verify command substitutions retain protection across nested parentheses.
#[test]
fn interpolate_command_rejects_markers_after_nested_subshells() {
    let error = interpolate_command_with_shell(
        &format!("echo \"$( (true); echo {INS_TOKEN} )\""),
        &[Utf8PathBuf::from("x; touch injected")],
        &[],
        RecipeShell::Posix,
    )
    .expect_err("markers inside nested command substitutions must be rejected");
    assert!(matches!(error, IrGenError::InvalidCommand { .. }));
}

/// Verify quoted parentheses do not close a command substitution early.
#[test]
fn interpolate_command_rejects_markers_after_quoted_command_substitution_parentheses() {
    for marker in [INS_TOKEN, OUTS_TOKEN] {
        let error = interpolate_command_with_shell(
            &format!("echo \"$(printf ')' ; echo {marker})\""),
            &[Utf8PathBuf::from("x;id;echo")],
            &[Utf8PathBuf::from("x;id;echo")],
            RecipeShell::Posix,
        )
        .expect_err("markers inside quoted command substitutions must be rejected");
        assert!(matches!(error, IrGenError::InvalidCommand { .. }));
    }
}
/// Verify POSIX command substitution rejects recipe markers before shell execution.
#[test]
fn interpolate_command_rejects_markers_in_command_substitutions() {
    for marker in [INS_TOKEN, OUTS_TOKEN] {
        for template in [
            format!("echo $(printf %s {marker})"),
            format!("echo \"$(printf %s {marker})\""),
        ] {
            let error = interpolate_command_with_shell(
                &template,
                &[Utf8PathBuf::from("in")],
                &[Utf8PathBuf::from("out")],
                RecipeShell::Posix,
            )
            .expect_err("markers inside command substitutions must be rejected");
            assert!(matches!(error, IrGenError::InvalidCommand { .. }));
        }
    }
}

/// Verify scripts apply the same quote-context safeguards as command recipes.
#[test]
fn interpolate_script_applies_quote_context_safeguards() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("in")],
        &[Utf8PathBuf::from("out")],
        RecipeShell::Posix,
    );
    for (marker, replacement) in [(INS_TOKEN, "in"), (OUTS_TOKEN, "out")] {
        for (template, expected_result) in [
            (format!("echo {marker}"), Ok(format!("echo {replacement}"))),
            (
                format!("echo '{marker}'"),
                Ok(format!("echo '{replacement}'")),
            ),
            (
                format!("echo \"{marker}\""),
                Ok(format!("echo \"{replacement}\"")),
            ),
            (format!("echo $(printf %s {marker})"), Err(())),
        ] {
            match expected_result {
                Ok(expected_command) => assert_eq!(
                    interpolate_script_with_bindings(&template, &bindings)
                        .expect("context-safe script should interpolate"),
                    expected_command
                ),
                Err(()) => assert!(matches!(
                    interpolate_script_with_bindings(&template, &bindings),
                    Err(IrGenError::InvalidCommand { .. })
                )),
            }
        }
    }
}

/// Verify comments cannot alter quote context for later command or script markers.
#[test]
fn comments_preserve_quote_context_for_later_markers() {
    let path = "x\"; touch injected; echo \"y";
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from(path)],
        &[Utf8PathBuf::from(path)],
        RecipeShell::Posix,
    );
    for comment in [
        "# unmatched apostrophe '\n",
        "# unmatched double quote \"\n",
    ] {
        let template = format!("{comment}echo \"{INS_TOKEN}\"");
        let expected = format!("{comment}echo \"x\\\"; touch injected; echo \\\"y\"");
        assert_eq!(
            interpolate_command_with_bindings(&template, &bindings)
                .expect("comments must not affect command marker quoting"),
            expected
        );
        assert_eq!(
            interpolate_script_with_bindings(&template, &bindings)
                .expect("comments must not affect script marker quoting"),
            expected
        );
    }
    let continuation_comment = "echo before \\\n# unmatched apostrophe '\n";
    let continuation_template = format!("{continuation_comment}echo \"{INS_TOKEN}\"");
    let continuation_expected =
        format!("{continuation_comment}echo \"x\\\"; touch injected; echo \\\"y\"");
    assert_eq!(
        interpolate_script_with_bindings(&continuation_template, &bindings)
            .expect("continued comment must not affect script marker quoting"),
        continuation_expected
    );
}

/// Verify comments preserve their markers while escaped boundaries remain executable text.
#[test]
fn comments_leave_markers_literal_only_after_unescaped_boundaries() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("input")],
        &[Utf8PathBuf::from("output")],
        RecipeShell::Posix,
    );
    let comment_template = format!("# {INS_TOKEN}\necho \"{OUTS_TOKEN}\"");
    let comment_expected = format!("# {INS_TOKEN}\necho \"output\"");
    assert_eq!(
        interpolate_script_with_bindings(&comment_template, &bindings)
            .expect("comment marker must remain literal"),
        comment_expected
    );
    for (template, expected) in [
        (
            format!("echo escaped\\ # {INS_TOKEN}"),
            "echo escaped\\ # input",
        ),
        (
            format!("echo escaped\\;# {OUTS_TOKEN}"),
            "echo escaped\\;# output",
        ),
    ] {
        assert_eq!(
            interpolate_script_with_bindings(&template, &bindings)
                .expect("escaped comment boundary must remain executable"),
            expected
        );
    }
}

/// Verify heredoc data remains literal and cannot alter later marker context.
#[test]
fn heredoc_bodies_preserve_quote_context_for_later_markers() {
    let path = "x\"; touch injected; echo \"y";
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from(path)],
        &[Utf8PathBuf::from(path)],
        RecipeShell::Posix,
    );
    for (delimiter, terminator) in [
        ("EOF", "EOF"),
        ("'EOF'", "EOF"),
        ("-EOF", "\tEOF"),
        ("''", ""),
    ] {
        let template = format!(
            "cat <<{delimiter}\nunmatched ' and \" {INS_TOKEN}\n{terminator}\necho \"{OUTS_TOKEN}\""
        );
        let expected = format!(
            "cat <<{delimiter}\nunmatched ' and \" {INS_TOKEN}\n{terminator}\necho \"x\\\"; touch injected; echo \\\"y\""
        );
        assert_eq!(
            interpolate_script_with_bindings(&template, &bindings)
                .expect("heredoc data must not affect later marker quoting"),
            expected
        );
    }
}

/// Verify markers in a heredoc declaration lower before its body becomes inert.
#[test]
fn heredoc_declaration_markers_are_lowered() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("input")],
        &[Utf8PathBuf::from("output")],
        RecipeShell::Posix,
    );
    let template = format!("cat <<{OUTS_TOKEN}\n{INS_TOKEN}\noutput\necho \"{INS_TOKEN}\"");
    let expected = format!("cat <<output\n{INS_TOKEN}\noutput\necho \"input\"");
    assert_eq!(
        interpolate_script_with_bindings(&template, &bindings)
            .expect("heredoc declaration marker must be lowered"),
        expected
    );
}

#[test]
fn power_shell_bindings_preserve_literal_backticks_in_paths() {
    assert_power_shell_path_interpolation(
        "source`file",
        "output`file",
        "Copy-Item 'source`file' 'output`file'",
        "PowerShell single-quoted paths should preserve literal backticks",
    );
}

/// Assert PowerShell interpolation for one input/output path pair.
fn assert_power_shell_path_interpolation(
    input_path: &str,
    output_path: &str,
    expected_command: &str,
    expect_message: &str,
) {
    let inputs = [Utf8PathBuf::from(input_path)];
    let outputs = [Utf8PathBuf::from(output_path)];
    let bindings = CommandBindings::new(&inputs, &outputs, RecipeShell::PowerShell);
    let interpolation = interpolate_command_with_bindings(
        &format!("Copy-Item {INS_TOKEN} {OUTS_TOKEN}"),
        &bindings,
    );
    assert!(interpolation.is_ok(), "{expect_message}: {interpolation:?}");
    let Ok(command) = interpolation else {
        return;
    };
    assert_eq!(command, expected_command);
}
/// Verify that PowerShell-specific escaped double quotes bypass POSIX syntax checks.
#[test]
fn power_shell_bindings_allow_escaped_double_quotes() {
    let bindings = CommandBindings::new(&[], &[], RecipeShell::PowerShell);
    let command = interpolate_command_with_bindings(r#"Write-Output "a`"b""#, &bindings)
        .expect("PowerShell escaped double quotes should interpolate");
    assert_eq!(command, r#"Write-Output "a`"b""#);
}

/// Verify PowerShell substitutes placeholders following native backtick escapes.
#[test]
fn power_shell_backticks_do_not_protect_command_placeholders() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("input")],
        &[Utf8PathBuf::from("output")],
        RecipeShell::PowerShell,
    );
    let command = interpolate_command_with_bindings(
        &format!("Write-Output `$in `$out `{INS_TOKEN}"),
        &bindings,
    )
    .expect("PowerShell placeholders following backticks should interpolate");

    assert_eq!(command, "Write-Output `'input' `'output' `'input'");
}

/// Verify PowerShell scripts preserve native backticks while substituting tokens.
#[test]
fn power_shell_backticks_do_not_protect_script_placeholders() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("input")],
        &[Utf8PathBuf::from("output")],
        RecipeShell::PowerShell,
    );
    let script = interpolate_script_with_bindings(
        &format!("Write-Output `$in `$out `{INS_TOKEN}"),
        &bindings,
    )
    .expect("PowerShell script placeholders following backticks should interpolate");

    assert_eq!(script, "Write-Output `'input' `'output' `'input'");
}

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
