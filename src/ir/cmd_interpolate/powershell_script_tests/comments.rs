//! Test PowerShell comment and scanner-state cases.

use super::*;
use super::{power_shell_output_case, power_shell_rejection_case, script_scenario};

/// Reject markers after here-string or braced-variable syntax the scanner cannot model.
#[rstest]
#[case::here_string_inside_command_substitution(script_scenario!(
    format!("Write-Output $(@'\n'\n'@)') {OUTS_TOKEN} '"),
    RecipeShell::PowerShell,
    vec![],
    vec![Utf8PathBuf::from(HOSTILE_SOURCE_PATH)],
    ScriptExpectation::Reject
))]
#[case::braced_variable_name(script_scenario!(
    format!("Write-Output ${{a'b}} '{OUTS_TOKEN}'"),
    RecipeShell::PowerShell,
    vec![],
    vec![Utf8PathBuf::from(HOSTILE_SOURCE_PATH)],
    ScriptExpectation::Reject
))]
fn power_shell_scripts_reject_markers_after_unmodelled_syntax(#[case] scenario: ScriptScenario) {
    scenario.verify();
}

/// Fail closed when a comment quote changes the shared PowerShell quote state.
#[test]
fn power_shell_script_comment_quotes_can_reject_later_markers() {
    power_shell_rejection_case!(format!(
        "# don't parse this comment\nWrite-Output {OUTS_TOKEN}"
    ))
    .verify();
}

/// Keep plain comment text from poisoning later unquoted marker sites.
#[test]
fn power_shell_scripts_interpolate_markers_after_plain_comments() {
    power_shell_output_case!(
        format!("# ordinary comment\nWrite-Output {OUTS_TOKEN}"),
        "# ordinary comment\nWrite-Output 'output file.txt'"
    )
    .verify();
}

/// Keep token-internal hashes from turning later source into a comment.
#[test]
fn power_shell_scripts_preserve_hashes_inside_argument_tokens() {
    power_shell_output_case!(
        format!("Write-Output file#tag; Write-Output \"ok\"; Write-Output {INS_TOKEN}"),
        "Write-Output file#tag; Write-Output \"ok\"; Write-Output '; New-Item pwned; Write-Output x'"
    )
    .verify();
}

/// Keep argument punctuation from turning a literal hash into a comment.
#[test]
fn power_shell_scripts_preserve_hash_after_argument_asterisk() {
    power_shell_output_case!(
        format!("Write-Output foo*#bar \"quoted\"; Write-Output {OUTS_TOKEN}"),
        "Write-Output foo*#bar \"quoted\"; Write-Output 'output file.txt'"
    )
    .verify();
}

/// Preserve hashes after assignment and bracket text in argument tokens.
#[rstest]
#[case::assignment(ScriptArgumentCase { fragment: "a=#b" })]
#[case::closing_bracket(ScriptArgumentCase { fragment: "a]#b" })]
fn power_shell_scripts_preserve_expression_hashes_inside_argument_tokens(
    #[case] argument: ScriptArgumentCase,
) {
    power_shell_output_case!(
        format!("Write-Output {} {OUTS_TOKEN}", argument.fragment),
        format!("Write-Output {} 'output file.txt'", argument.fragment)
    )
    .verify();
}

/// Ignore block-comment delimiters that occur inside a single-line comment.
#[test]
fn power_shell_scripts_ignore_block_opener_inside_line_comment() {
    power_shell_output_case!(
        format!("# <# note\nWrite-Output {OUTS_TOKEN}"),
        "# <# note\nWrite-Output 'output file.txt'"
    )
    .verify();
}
