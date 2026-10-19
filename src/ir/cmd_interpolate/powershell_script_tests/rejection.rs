//! Test PowerShell marker rejection contexts.

use super::*;

/// Reject every PowerShell script marker in contexts that need shell parsing.
#[rstest]
#[case::single_quoted_input(power_shell_rejection_case!(format!("Write-Output '{INS_TOKEN}' > {OUTS_TOKEN}")))]
#[case::single_quoted_output(power_shell_rejection_case!(format!("Write-Output {INS_TOKEN} > '{OUTS_TOKEN}'")))]
#[case::double_quoted_input(power_shell_rejection_case!(format!("Write-Output \"{INS_TOKEN}\" > {OUTS_TOKEN}")))]
#[case::double_quoted_output(power_shell_rejection_case!(format!("Write-Output {INS_TOKEN} > \"{OUTS_TOKEN}\"")))]
#[case::command_substitution_input(power_shell_rejection_case!(format!("Write-Output $(Write-Output {INS_TOKEN}) > {OUTS_TOKEN}")))]
#[case::command_substitution_output(power_shell_rejection_case!(format!("Write-Output {INS_TOKEN} > $(Write-Output {OUTS_TOKEN})")))]
#[case::escaped_input(power_shell_rejection_case!(format!("Write-Output `{INS_TOKEN} > {OUTS_TOKEN}")))]
#[case::escaped_output(power_shell_rejection_case!(format!("Write-Output {INS_TOKEN} > `{OUTS_TOKEN}")))]
#[case::doubled_single_quote(power_shell_rejection_case!(format!("Write-Output 'prefix ''{INS_TOKEN}' > {OUTS_TOKEN}")))]
#[case::doubled_double_quote(power_shell_rejection_case!(format!("Write-Output \"prefix \"\"{INS_TOKEN}\" > {OUTS_TOKEN}")))]
#[case::single_quoted_here_string_input(power_shell_rejection_case!(format!("@'\n{INS_TOKEN}\n'@ > {OUTS_TOKEN}")))]
#[case::single_quoted_here_string_output(power_shell_rejection_case!(format!("@'\n{OUTS_TOKEN}\n'@ > {INS_TOKEN}")))]
#[case::mid_token_here_string_lookalike(power_shell_rejection_case!(format!("Write-Output foo@'\nbar' ; baz\n'@ {INS_TOKEN}'")))]
#[case::single_quoted_here_string_space_after_opener(power_shell_rejection_case!(format!("@' \n{INS_TOKEN}\n'@")))]
#[case::single_quoted_here_string_tab_after_opener(power_shell_rejection_case!(format!("@'\t\n{INS_TOKEN}\n'@")))]
#[case::single_quoted_here_string_crlf(power_shell_rejection_case!(format!("@'\r\nbody ' quote {INS_TOKEN}\r\n'@")))]
#[case::double_quoted_here_string_input(power_shell_rejection_case!(format!("@\"\n{INS_TOKEN}\n\"@ > {OUTS_TOKEN}")))]
#[case::double_quoted_here_string_output(power_shell_rejection_case!(format!("@\"\n{OUTS_TOKEN}\n\"@ > {INS_TOKEN}")))]
#[case::double_quoted_here_string_space_after_opener(power_shell_rejection_case!(format!("@\" \n{INS_TOKEN}\n\"@")))]
#[case::double_quoted_here_string_tab_after_opener(power_shell_rejection_case!(format!("@\"\t\n{INS_TOKEN}\n\"@")))]
#[case::double_quoted_here_string_crlf(power_shell_rejection_case!(format!("@\"\r\nbody \" quote {OUTS_TOKEN}\r\n\"@")))]
#[case::single_quoted_here_string_after_literal_quote(power_shell_rejection_case!(format!("@'\nbody ' quote {INS_TOKEN}\n'@")))]
#[case::double_quoted_here_string_after_literal_quote(power_shell_rejection_case!(format!("@\"\nbody \" quote {OUTS_TOKEN}\n\"@")))]
#[case::stop_parsing_token(power_shell_rejection_case!(format!("cmd /c --% type {INS_TOKEN}")))]
#[case::stop_parsing_after_hash_in_argument(power_shell_rejection_case!(format!("cmd /c echo prefix#suffix --% {INS_TOKEN}")))]
#[case::stop_parsing_after_at_hash_in_argument(power_shell_rejection_case!(format!("cmd /c echo prefix@#suffix --% {INS_TOKEN}")))]
#[case::stop_parsing_after_semicolon(power_shell_rejection_case!(format!("cmd /c --% literal; Write-Output {INS_TOKEN}")))]
#[case::quoted_pipe_in_stop_parsing_region(power_shell_rejection_case!(format!("cmd /c --% echo \"prefix | {INS_TOKEN}\"")))]
#[case::stop_parsing_single_quote_before_pipeline(power_shell_rejection_case!(format!(
    "cmd /c --% 'prefix | text' | Write-Output {OUTS_TOKEN}"
)))]
#[case::stop_parsing_backtick_before_quote_and_pipeline(power_shell_rejection_case!(format!(
    "cmd /c --% \"prefix`\" still \" | Write-Output {OUTS_TOKEN}"
)))]
#[case::typographic_quote_before_pipe_in_stop_parsing_region(power_shell_rejection_case!(format!(
    "cmd /c --% echo “ | Write-Output {OUTS_TOKEN}"
)))]
#[case::typographic_here_string_terminator(power_shell_rejection_case!(format!(
    "@'\n’@ + \"\n'@\nWrite-Output {INS_TOKEN}\n\""
)))]
fn power_shell_scripts_reject_protected_markers(#[case] scenario: ScriptScenario) {
    scenario.verify();
}

/// Reject markers whose standalone literals would merge with adjacent strings.
#[rstest]
#[case::after_single_quoted_input(format!("Write-Output 'prefix'{INS_TOKEN}"))]
#[case::before_single_quoted_input(format!("Write-Output {INS_TOKEN}'suffix'"))]
#[case::after_single_quoted_output(format!("Write-Output 'prefix'{OUTS_TOKEN}"))]
#[case::trailing_typographic_single_quote(format!("Write-Output {INS_TOKEN}’suffix’"))]
#[case::adjacent_input_then_output(format!("Write-Output {INS_TOKEN}{OUTS_TOKEN}"))]
#[case::adjacent_output_then_input(format!("Write-Output {OUTS_TOKEN}{INS_TOKEN}"))]
fn power_shell_scripts_reject_markers_at_literal_boundaries(#[case] template: String) {
    power_shell_rejection_case!(template).verify();
}

/// Reject markers after comment syntax could desynchronize the partial scanner.
#[rstest]
#[case::comment_quote(power_shell_rejection_case!(format!("# '\nWrite-Output '{INS_TOKEN}'")))]
#[case::comment_after_assignment_input(power_shell_rejection_case!(format!("$x=#'\nWrite-Output '{INS_TOKEN}'")))]
#[case::comment_after_assignment_output(power_shell_rejection_case!(format!("$x=#'\nWrite-Output '{OUTS_TOKEN}'")))]
#[case::comment_after_variable_input(power_shell_rejection_case!(format!("$value#'\nWrite-Output '{INS_TOKEN}'")))]
#[case::comment_after_variable_output(power_shell_rejection_case!(format!("$value#'\nSet-Content output '{OUTS_TOKEN}'")))]
#[case::comment_after_postfix_increment(power_shell_rejection_case!(format!("$i++#'\nWrite-Output '{OUTS_TOKEN}'")))]
#[case::comment_after_member_postfix_increment(power_shell_rejection_case!(format!(
    "$obj.Value++#'\nWrite-Output '{INS_TOKEN}'"
)))]
#[case::comment_after_member_postfix_decrement(power_shell_rejection_case!(format!(
    "$obj.Value--#'\nWrite-Output '{OUTS_TOKEN}'"
)))]
#[case::comment_after_indexed_postfix_increment(power_shell_rejection_case!(format!(
    "$arr[0]++#'\nWrite-Output '{INS_TOKEN}'"
)))]
#[case::comment_after_member_indexed_postfix_decrement(power_shell_rejection_case!(format!(
    "$obj.Items[0]--#'\nWrite-Output '{OUTS_TOKEN}'"
)))]
#[case::comment_after_assignment_stop_parsing_token(power_shell_rejection_case!(format!(
    "$x=# --% raw comment text\nWrite-Output {OUTS_TOKEN}"
)))]
#[case::comment_after_type_literal(power_shell_rejection_case!(format!("[int]#'\nWrite-Output '{OUTS_TOKEN}'")))]
#[case::comment_stop_parsing_token(power_shell_rejection_case!(format!("# --% raw comment text\nWrite-Output {OUTS_TOKEN}")))]
#[case::block_comment_after_token_hash(power_shell_rejection_case!(format!(
    "Write-Output a#b <#\n\"\n#>\nWrite-Output \"{INS_TOKEN}\""
)))]
#[case::comment_closes_substitution(power_shell_rejection_case!(format!(
    "Write-Output $(Write-Output x # )\n{OUTS_TOKEN})"
)))]
fn power_shell_scripts_reject_markers_after_ambiguous_comment_syntax(
    #[case] scenario: ScriptScenario,
) {
    scenario.verify();
}

/// Reject marker sites protected by PowerShell's typographic quote delimiters.
#[rstest]
#[case::left_right_single(QuotePair { opening: "‘", closing: "’" })]
#[case::low_single(QuotePair { opening: "‚", closing: "‚" })]
#[case::reversed_single(QuotePair { opening: "‛", closing: "‛" })]
#[case::left_right_double(QuotePair { opening: "“", closing: "”" })]
#[case::low_double(QuotePair { opening: "„", closing: "‟" })]
#[case::double_high_reversed(QuotePair { opening: "‟", closing: "‟" })]
fn power_shell_scripts_reject_typographic_quote_markers(#[case] pair: QuotePair) {
    let template = format!(
        "Write-Output {}{INS_TOKEN}{} > {OUTS_TOKEN}",
        pair.opening, pair.closing
    );
    power_shell_rejection_case!(template).verify();
}
