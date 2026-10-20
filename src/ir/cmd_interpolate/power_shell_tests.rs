//! Test PowerShell recipe-marker protection contexts for commands and scripts.

use super::*;

use camino::Utf8PathBuf;
use rstest::rstest;

#[derive(Clone, Copy)]
enum PowerShellRejectionBindings {
    HostileInputAndOutput,
    OutputOnly,
    HostileInputOnly,
}

struct PowerShellRejectionScenario {
    template: String,
    paths: PowerShellRejectionBindings,
}

impl PowerShellRejectionScenario {
    /// Prepare the paths required by this rejection case.
    fn bindings(&self) -> CommandBindings {
        let hostile_path = Utf8PathBuf::from("; New-Item pwned; Write-Output x");
        let output_path = Utf8PathBuf::from("out");
        let (inputs, outputs) = match self.paths {
            PowerShellRejectionBindings::HostileInputAndOutput => {
                (vec![hostile_path], vec![output_path])
            }
            PowerShellRejectionBindings::OutputOnly => (vec![], vec![output_path]),
            PowerShellRejectionBindings::HostileInputOnly => (vec![hostile_path], vec![]),
        };
        CommandBindings::new(&inputs, &outputs, RecipeShell::PowerShell)
    }

    /// Require command and script interpolation to reject this same template.
    fn verify_command_and_script(self) {
        let bindings = self.bindings();
        assert_rejected_template(
            interpolate_command_with_bindings(&self.template, &bindings),
            &self.template,
        );
        assert_rejected_template(
            interpolate_script_with_bindings(&self.template, &bindings),
            &self.template,
        );
    }
}

/// Assert rejection while preserving the unexpanded diagnostic template.
fn assert_rejected_template(result: Result<String, IrGenError>, template: &str) {
    let error = result.expect_err("scanner-sensitive contexts must reject markers");
    assert!(
        matches!(error, IrGenError::InvalidCommand { command, .. } if command == template),
        "the diagnostic should retain the original template"
    );
}

/// Reject shared command and script markers whose literals would merge with strings.
#[rstest]
#[case::single_quoted_input(format!("Write-Output '{INS_TOKEN}' > {OUTS_TOKEN}"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::single_quoted_output(format!("Write-Output {INS_TOKEN} > '{OUTS_TOKEN}'"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::double_quoted_input(format!("Write-Output \"{INS_TOKEN}\" > {OUTS_TOKEN}"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::double_quoted_output(format!("Write-Output {INS_TOKEN} > \"{OUTS_TOKEN}\""), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::command_substitution_input(format!("Write-Output $(Write-Output {INS_TOKEN}) > {OUTS_TOKEN}"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::command_substitution_output(format!("Write-Output {INS_TOKEN} > $(Write-Output {OUTS_TOKEN})"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::escaped_input(format!("Write-Output `{INS_TOKEN} > {OUTS_TOKEN}"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::escaped_output(format!("Write-Output {INS_TOKEN} > `{OUTS_TOKEN}"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::doubled_single_quote(format!("Write-Output 'prefix ''{INS_TOKEN}' > {OUTS_TOKEN}"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::doubled_double_quote(format!("Write-Output \"prefix \"\"{INS_TOKEN}\" > {OUTS_TOKEN}"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::after_single_quoted_input(format!("Write-Output 'prefix'{INS_TOKEN}"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::before_single_quoted_input(format!("Write-Output {INS_TOKEN}'suffix'"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::after_single_quoted_output(format!("Write-Output 'prefix'{OUTS_TOKEN}"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::trailing_typographic_single_quote(format!("Write-Output {INS_TOKEN}’suffix’"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::adjacent_input_then_output(format!("Write-Output {INS_TOKEN}{OUTS_TOKEN}"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::adjacent_output_then_input(format!("Write-Output {OUTS_TOKEN}{INS_TOKEN}"), PowerShellRejectionBindings::HostileInputAndOutput)]
fn power_shell_commands_and_scripts_reject_shared_literal_boundaries(
    #[case] template: String,
    #[case] paths: PowerShellRejectionBindings,
) {
    PowerShellRejectionScenario { template, paths }.verify_command_and_script();
}

/// Reject shared markers after ambiguous comment and stop-parsing syntax.
#[rstest]
#[case::ambiguous_comment_input(format!("$value#'\nWrite-Output '{INS_TOKEN}'"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::ambiguous_comment_output(format!("$value#'\nSet-Content output '{OUTS_TOKEN}'"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::postfix_operator_comment(format!("$i++#'\nWrite-Output '{OUTS_TOKEN}'"), PowerShellRejectionBindings::OutputOnly)]
#[case::addition_operator_comment(format!("$value = 1 +# 'comment'\nWrite-Output {OUTS_TOKEN}"), PowerShellRejectionBindings::OutputOnly)]
#[case::addition_operator_without_assignment(format!("1 +#'\n2; Write-Output 'prefix {INS_TOKEN} suffix'"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::multiplication_operator_comment(format!("$value = 2 *# 'comment'\nWrite-Output {OUTS_TOKEN}"), PowerShellRejectionBindings::OutputOnly)]
#[case::indexed_member_comment(format!("$n=$s[0].Length#'comment'\nWrite-Output 'prefix {INS_TOKEN} suffix'"), PowerShellRejectionBindings::HostileInputOnly)]
#[case::redirection_operator_comment(format!("Write-Output value ># 'comment'\nWrite-Output {OUTS_TOKEN}"), PowerShellRejectionBindings::OutputOnly)]
#[case::compact_addition_hash_quote(format!("$x=1+#'\nWrite-Output 'prefix {OUTS_TOKEN} suffix'"), PowerShellRejectionBindings::OutputOnly)]
#[case::compact_member_hash_quote(format!("$n=$s.Length#\"\nWrite-Output \"prefix {OUTS_TOKEN} suffix\""), PowerShellRejectionBindings::OutputOnly)]
#[case::compact_hash_backtick(format!("$x=1+#`'\nWrite-Output 'prefix {OUTS_TOKEN} suffix'"), PowerShellRejectionBindings::OutputOnly)]
#[case::compact_hash_substitution(format!("$n=$s.Length#$(\nWrite-Output {OUTS_TOKEN})"), PowerShellRejectionBindings::OutputOnly)]
#[case::numeric_literal_hash_quote(format!("$x=5#'\nWrite-Output '{INS_TOKEN}'"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::signed_numeric_literal_hash_quote(format!("$x=-5#'\nWrite-Output 'prefix {INS_TOKEN} suffix'"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::hexadecimal_literal_hash_quote(format!("$x=0x10#'\nWrite-Output '{OUTS_TOKEN}'"), PowerShellRejectionBindings::OutputOnly)]
#[case::hexadecimal_digit_d_hash_quote(format!("$x=0xd#'\nWrite-Output 'prefix {INS_TOKEN} suffix'"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::suffixed_hexadecimal_literal_hash_quote(format!("$x=0x12Lpb#'\nWrite-Output 'prefix {INS_TOKEN} suffix'"), PowerShellRejectionBindings::HostileInputAndOutput)]
#[case::size_literal_hash_quote(format!("$x=1kb#'\nWrite-Output '{OUTS_TOKEN}'"), PowerShellRejectionBindings::OutputOnly)]
#[case::stop_parsing_token(format!("cmd /c --% type {INS_TOKEN}"), PowerShellRejectionBindings::HostileInputOnly)]
#[case::stop_parsing_single_quote(format!("cmd /c --% 'prefix | text' | Write-Output {OUTS_TOKEN}"), PowerShellRejectionBindings::OutputOnly)]
#[case::stop_parsing_backtick_before_quote(format!("cmd /c --% \"prefix`\" still \" | Write-Output {OUTS_TOKEN}"), PowerShellRejectionBindings::OutputOnly)]
fn power_shell_commands_and_scripts_reject_shared_ambiguous_syntax(
    #[case] template: String,
    #[case] paths: PowerShellRejectionBindings,
) {
    PowerShellRejectionScenario { template, paths }.verify_command_and_script();
}

/// Preserve an argument hash after punctuation that remains part of its token.
#[test]
fn power_shell_commands_preserve_hash_after_argument_asterisk() {
    let template = format!("Write-Output foo*#bar {OUTS_TOKEN}");
    let bindings = CommandBindings::new(&[], &[Utf8PathBuf::from("out")], RecipeShell::PowerShell);
    let actual = interpolate_command_with_bindings(&template, &bindings)
        .expect("an asterisk inside an argument does not start a comment");

    assert_eq!(actual, "Write-Output foo*#bar 'out'");
}

/// Preserve a generic argument hash before a balanced quoted argument in scripts and commands.
#[test]
fn power_shell_scripts_and_commands_preserve_generic_argument_hashes() {
    let template = format!("Write-Output foo*#'balanced'; Write-Output {OUTS_TOKEN}");
    let bindings = CommandBindings::new(&[], &[Utf8PathBuf::from("out")], RecipeShell::PowerShell);

    let command = interpolate_command_with_bindings(&template, &bindings)
        .expect("a generic argument hash does not begin a comment");
    let script = interpolate_script_with_bindings(&template, &bindings)
        .expect("PowerShell scripts use the same unquoted insertion rule");
    let expected = "Write-Output foo*#'balanced'; Write-Output 'out'";

    assert_eq!(command, expected);
    assert_eq!(script, expected);
}

/// Preserve hashes after argument characters that do not start comments.
#[rstest]
#[case::assignment("a=#b")]
#[case::closing_bracket("a]#b")]
#[case::postfix_operator("prefix++#suffix")]
fn power_shell_commands_preserve_hashes_inside_argument_tokens(#[case] argument: &str) {
    let template = format!("Write-Output {argument} {OUTS_TOKEN}");
    let bindings = CommandBindings::new(&[], &[Utf8PathBuf::from("out")], RecipeShell::PowerShell);
    let actual = interpolate_command_with_bindings(&template, &bindings)
        .expect("argument punctuation does not turn its hash into a comment");

    assert_eq!(actual, format!("Write-Output {argument} 'out'"));
}
