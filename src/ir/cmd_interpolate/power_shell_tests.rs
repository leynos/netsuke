//! Test PowerShell recipe-marker protection contexts for commands and scripts.

use super::*;

use camino::Utf8PathBuf;
use rstest::rstest;

enum PowerShellCommandPathBindings {
    HostileInputAndOutput,
    OutputOnly,
    HostileInputOnly,
}

struct PowerShellCommandRejectionScenario {
    template: String,
    paths: PowerShellCommandPathBindings,
}

impl PowerShellCommandRejectionScenario {
    /// Interpolate a command and require an error retaining its original template.
    fn verify(self) {
        let hostile_path = Utf8PathBuf::from("; New-Item pwned; Write-Output x");
        let output_path = Utf8PathBuf::from("out");
        let bindings = match self.paths {
            PowerShellCommandPathBindings::HostileInputAndOutput => {
                CommandBindings::new(&[hostile_path], &[output_path], RecipeShell::PowerShell)
            }
            PowerShellCommandPathBindings::OutputOnly => {
                CommandBindings::new(&[], &[output_path], RecipeShell::PowerShell)
            }
            PowerShellCommandPathBindings::HostileInputOnly => {
                CommandBindings::new(&[hostile_path], &[], RecipeShell::PowerShell)
            }
        };
        let error = interpolate_command_with_bindings(&self.template, &bindings)
            .expect_err("scanner-sensitive contexts must reject command markers");

        assert!(
            matches!(error, IrGenError::InvalidCommand { command, .. } if command == self.template),
            "the diagnostic should retain the original template"
        );
    }
}

macro_rules! command_rejection_case {
    ($template:expr, $paths:expr) => {
        PowerShellCommandRejectionScenario {
            template: $template,
            paths: $paths,
        }
    };
}

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

/// Reject markers after comment and stop-parsing syntax that the scanner cannot model.
#[rstest]
#[case::ambiguous_comment_input(
    command_rejection_case!(format!("$value#'\nWrite-Output '{INS_TOKEN}'"), PowerShellCommandPathBindings::HostileInputAndOutput)
)]
#[case::ambiguous_comment_output(
    command_rejection_case!(format!("$value#'\nSet-Content output '{OUTS_TOKEN}'"), PowerShellCommandPathBindings::HostileInputAndOutput)
)]
#[case::postfix_operator_comment(
    command_rejection_case!(format!("$i++#'\nWrite-Output '{OUTS_TOKEN}'"), PowerShellCommandPathBindings::OutputOnly)
)]
#[case::stop_parsing_token(
    command_rejection_case!(format!("cmd /c --% type {INS_TOKEN}"), PowerShellCommandPathBindings::HostileInputOnly)
)]
#[case::stop_parsing_single_quote(
    command_rejection_case!(format!("cmd /c --% 'prefix | text' | Write-Output {OUTS_TOKEN}"), PowerShellCommandPathBindings::OutputOnly)
)]
#[case::stop_parsing_backtick_before_quote(
    command_rejection_case!(format!("cmd /c --% \"prefix`\" still \" | Write-Output {OUTS_TOKEN}"), PowerShellCommandPathBindings::OutputOnly)
)]
fn power_shell_commands_reject_markers_after_ambiguous_syntax(
    #[case] scenario: PowerShellCommandRejectionScenario,
) {
    scenario.verify();
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
