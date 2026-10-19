//! Test PowerShell native-command stop-parsing cases.

use super::*;
use super::{power_shell_output_case, script_scenario};

/// Resume safe interpolation after PowerShell stop parsing ends.
#[rstest]
#[case::newline(power_shell_output_case!(format!("cmd /c --% literal\nWrite-Output {OUTS_TOKEN}"), "cmd /c --% literal\nWrite-Output 'output file.txt'"))]
#[case::pipeline(power_shell_output_case!(format!("cmd /c --% echo \"prefix | text\" | Write-Output {OUTS_TOKEN}"), "cmd /c --% echo \"prefix | text\" | Write-Output 'output file.txt'"))]
fn power_shell_scripts_resume_after_stop_parsing(#[case] scenario: ScriptScenario) {
    scenario.verify();
}

/// Do not treat `--%` embedded in an ordinary bare argument as a stop token.
#[rstest]
#[case::embedded_middle(power_shell_output_case!(format!("Write-Output prefix--%suffix {OUTS_TOKEN}"), "Write-Output prefix--%suffix 'output file.txt'"))]
#[case::embedded_start(power_shell_output_case!(format!("Write-Output --%suffix {OUTS_TOKEN}"), "Write-Output --%suffix 'output file.txt'"))]
#[case::after_hash_in_bare_argument(power_shell_output_case!(format!("Write-Output prefix#--%suffix {OUTS_TOKEN}"), "Write-Output prefix#--%suffix 'output file.txt'"))]
fn power_shell_scripts_accept_non_token_stop_parsing_text(#[case] scenario: ScriptScenario) {
    scenario.verify();
}

/// Reject markers whose quote context can be hidden by a multiline block comment.
#[rstest]
#[case::windows_semicolon_path(Utf8PathBuf::from(HOSTILE_SOURCE_PATH))]
#[case::command_substitution_path(Utf8PathBuf::from("$(New-Item pwned)"))]
fn power_shell_scripts_reject_markers_after_block_comment_openers(#[case] input_path: Utf8PathBuf) {
    let template = format!("<#\n\"\n#>\nWrite-Output \"{INS_TOKEN}\"");
    script_scenario!(
        template,
        RecipeShell::PowerShell,
        vec![input_path],
        vec![Utf8PathBuf::from(OUTPUT_PATH)],
        ScriptExpectation::Reject
    )
    .verify();
}

/// Accept unquoted markers and encode the semicolon path as one PowerShell word.
#[test]
fn power_shell_scripts_quote_unquoted_paths_in_multiline_text() {
    power_shell_output_case!(
        format!("Copy-Item {INS_TOKEN} {OUTS_TOKEN}\nWrite-Output done"),
        "Copy-Item '; New-Item pwned; Write-Output x' 'output file.txt'\nWrite-Output done"
    )
    .verify();
}
