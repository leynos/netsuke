//! Test PowerShell recipe-marker protection contexts for commands and scripts.

use super::*;

use camino::Utf8PathBuf;
use rstest::rstest;

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

/// Verify a PowerShell escape cannot hide a recipe marker from validation.
#[test]
fn power_shell_rejects_markers_after_backticks() {
    let bindings = CommandBindings::new(&[], &[Utf8PathBuf::from("out")], RecipeShell::PowerShell);
    let error =
        interpolate_command_with_bindings(&format!("Write-Output `{OUTS_TOKEN}"), &bindings)
            .expect_err("PowerShell backticks before markers must not bypass validation");
    assert!(matches!(error, IrGenError::InvalidCommand { .. }));
}

/// Reject a command marker in PowerShell's raw native-command argument region.
#[test]
fn power_shell_commands_reject_markers_after_stop_parsing_token() {
    let template = format!("cmd /c --% type {INS_TOKEN}");
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("; New-Item pwned; Write-Output x")],
        &[],
        RecipeShell::PowerShell,
    );
    let error = interpolate_command_with_bindings(&template, &bindings)
        .expect_err("raw native-command arguments cannot contain path markers");

    assert!(matches!(error, IrGenError::InvalidCommand { command, .. } if command == template));
}

/// Prepare bindings with a Windows-valid path that can expose source injection.
fn script_bindings() -> CommandBindings {
    CommandBindings::new(
        &[Utf8PathBuf::from("; New-Item pwned; Write-Output x")],
        &[Utf8PathBuf::from("output file.txt")],
        RecipeShell::PowerShell,
    )
}

/// Reject every PowerShell script marker in contexts that need shell parsing.
#[rstest]
#[case::single_quoted_input(format!("Write-Output '{INS_TOKEN}' > {OUTS_TOKEN}"))]
#[case::single_quoted_output(format!("Write-Output {INS_TOKEN} > '{OUTS_TOKEN}'"))]
#[case::double_quoted_input(format!("Write-Output \"{INS_TOKEN}\" > {OUTS_TOKEN}"))]
#[case::double_quoted_output(format!("Write-Output {INS_TOKEN} > \"{OUTS_TOKEN}\""))]
#[case::command_substitution_input(format!("Write-Output $(Write-Output {INS_TOKEN}) > {OUTS_TOKEN}"))]
#[case::command_substitution_output(format!("Write-Output {INS_TOKEN} > $(Write-Output {OUTS_TOKEN})"))]
#[case::escaped_input(format!("Write-Output `{INS_TOKEN} > {OUTS_TOKEN}"))]
#[case::escaped_output(format!("Write-Output {INS_TOKEN} > `{OUTS_TOKEN}"))]
#[case::doubled_single_quote(format!("Write-Output 'prefix ''{INS_TOKEN}' > {OUTS_TOKEN}"))]
#[case::doubled_double_quote(format!("Write-Output \"prefix \"\"{INS_TOKEN}\" > {OUTS_TOKEN}"))]
#[case::single_quoted_here_string_input(format!("@'\n{INS_TOKEN}\n'@ > {OUTS_TOKEN}"))]
#[case::single_quoted_here_string_output(format!("@'\n{OUTS_TOKEN}\n'@ > {INS_TOKEN}"))]
#[case::single_quoted_here_string_space_after_opener(format!("@' \n{INS_TOKEN}\n'@"))]
#[case::single_quoted_here_string_tab_after_opener(format!("@'\t\n{INS_TOKEN}\n'@"))]
#[case::single_quoted_here_string_crlf(format!("@'\r\nbody ' quote {INS_TOKEN}\r\n'@"))]
#[case::double_quoted_here_string_input(format!("@\"\n{INS_TOKEN}\n\"@ > {OUTS_TOKEN}"))]
#[case::double_quoted_here_string_output(format!("@\"\n{OUTS_TOKEN}\n\"@ > {INS_TOKEN}"))]
#[case::double_quoted_here_string_space_after_opener(format!("@\" \n{INS_TOKEN}\n\"@"))]
#[case::double_quoted_here_string_tab_after_opener(format!("@\"\t\n{INS_TOKEN}\n\"@"))]
#[case::double_quoted_here_string_crlf(format!("@\"\r\nbody \" quote {OUTS_TOKEN}\r\n\"@"))]
#[case::single_quoted_here_string_after_literal_quote(format!("@'\nbody ' quote {INS_TOKEN}\n'@"))]
#[case::double_quoted_here_string_after_literal_quote(format!("@\"\nbody \" quote {OUTS_TOKEN}\n\"@"))]
#[case::stop_parsing_token(format!("cmd /c --% type {INS_TOKEN}"))]
#[case::stop_parsing_after_hash_in_argument(format!("cmd /c echo prefix#suffix --% {INS_TOKEN}"))]
#[case::stop_parsing_after_at_hash_in_argument(format!("cmd /c echo prefix@#suffix --% {INS_TOKEN}"))]
#[case::stop_parsing_after_semicolon(format!("cmd /c --% literal; Write-Output {INS_TOKEN}"))]
#[case::quoted_pipe_in_stop_parsing_region(format!("cmd /c --% echo \"prefix | {INS_TOKEN}\""))]
fn power_shell_scripts_reject_protected_markers(#[case] template: String) {
    let result = interpolate_script_with_bindings(&template, &script_bindings());
    let error = result.expect_err("PowerShell scripts must reject protected markers");
    let IrGenError::InvalidCommand { command, .. } = error else {
        panic!("expected InvalidCommand for {template:?}, got {error:?}");
    };

    assert_eq!(
        command, template,
        "the diagnostic should retain the template"
    );
}

/// Reject markers after comment syntax could desynchronize the partial scanner.
#[rstest]
#[case::comment_quote(format!("# '\nWrite-Output '{INS_TOKEN}'"))]
#[case::comment_after_assignment_input(format!("$x=#'\nWrite-Output '{INS_TOKEN}'"))]
#[case::comment_after_assignment_output(format!("$x=#'\nWrite-Output '{OUTS_TOKEN}'"))]
#[case::comment_closes_substitution(format!(
    "Write-Output $(Write-Output x # )\n{OUTS_TOKEN})"
))]
fn power_shell_scripts_reject_markers_after_ambiguous_comment_syntax(#[case] template: String) {
    let error = interpolate_script_with_bindings(&template, &script_bindings())
        .expect_err("scanner-sensitive comment syntax must fail closed");

    assert!(matches!(error, IrGenError::InvalidCommand { command, .. } if command == template));
}

/// Reject marker sites protected by PowerShell's typographic quote delimiters.
#[rstest]
#[case::left_right_single("‘", "’")]
#[case::low_single("‚", "‚")]
#[case::reversed_single("‛", "‛")]
#[case::left_right_double("“", "”")]
#[case::low_double("„", "‟")]
#[case::double_high_reversed("‟", "‟")]
fn power_shell_scripts_reject_typographic_quote_markers(#[case] open: &str, #[case] close: &str) {
    let template = format!("Write-Output {open}{INS_TOKEN}{close} > {OUTS_TOKEN}");
    let error = interpolate_script_with_bindings(&template, &script_bindings())
        .expect_err("typographic PowerShell quotes must fail closed around markers");
    assert!(matches!(error, IrGenError::InvalidCommand { command, .. } if command == template));
}

/// Preserve PowerShell variables instead of treating their names as markers.
#[test]
fn power_shell_scripts_preserve_shell_variables_without_path_data() {
    let template =
        "Write-Output '$in' > $out\nWrite-Output '$out' '$ins' '$outs' $in $out $ins $outs";
    let actual = interpolate_script_with_bindings(template, &script_bindings())
        .expect("PowerShell variables are ordinary script text");

    assert_eq!(actual, template);
    assert!(!actual.contains("pwned"));
    assert!(!actual.contains("output file.txt"));
}

/// Encode apostrophes in paths as PowerShell single-quoted literal escapes.
#[test]
fn power_shell_scripts_double_apostrophes_in_unquoted_paths() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("source's file")],
        &[Utf8PathBuf::from("output's file")],
        RecipeShell::PowerShell,
    );
    let actual =
        interpolate_script_with_bindings(&format!("Copy-Item {INS_TOKEN} {OUTS_TOKEN}"), &bindings)
            .expect("PowerShell apostrophes should be doubled inside literal paths");

    assert_eq!(actual, "Copy-Item 'source''s file' 'output''s file'");
}

/// Keep the right single quotation mark as path data in an unquoted site.
#[test]
fn power_shell_scripts_quote_unicode_apostrophes_as_path_data() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("source’s file")],
        &[],
        RecipeShell::PowerShell,
    );
    let actual = interpolate_script_with_bindings(&format!("Copy-Item {INS_TOKEN}"), &bindings)
        .expect("a Unicode right single quotation mark is ordinary path data");

    assert_eq!(actual, "Copy-Item 'source’’s file'");
}

/// Double a typographic apostrophe in a hostile path before PowerShell parses it.
#[test]
fn power_shell_scripts_escape_typographic_apostrophes_in_path_data() {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("; New-Item pwned’ ; Write-Output x")],
        &[],
        RecipeShell::PowerShell,
    );
    let actual = interpolate_script_with_bindings(&format!("Copy-Item {INS_TOKEN}"), &bindings)
        .expect("the PowerShell literal encoder must preserve typographic apostrophes");

    assert_eq!(actual, "Copy-Item '; New-Item pwned’’ ; Write-Output x'");
}

/// Keep quoted PowerShell marker rejection distinct from POSIX quoting support.
#[rstest]
#[case::single_quoted_input(
    format!("Write-Output '{INS_TOKEN}' > {OUTS_TOKEN}"),
    "Write-Output '; New-Item pwned; Write-Output x' > output' file.txt'",
)]
#[case::single_quoted_output(
    format!("Write-Output {INS_TOKEN} > '{OUTS_TOKEN}'"),
    "Write-Output '; New-Item pwned; Write-Output x' > 'output file.txt'",
)]
#[case::double_quoted_input(
    format!("Write-Output \"{INS_TOKEN}\" > {OUTS_TOKEN}"),
    "Write-Output \"; New-Item pwned; Write-Output x\" > output' file.txt'",
)]
#[case::double_quoted_output(
    format!("Write-Output {INS_TOKEN} > \"{OUTS_TOKEN}\""),
    "Write-Output '; New-Item pwned; Write-Output x' > \"output file.txt\"",
)]
fn posix_scripts_still_interpolate_quoted_markers(
    #[case] template: String,
    #[case] expected: &str,
) {
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from("; New-Item pwned; Write-Output x")],
        &[Utf8PathBuf::from("output file.txt")],
        RecipeShell::Posix,
    );
    let actual = interpolate_script_with_bindings(&template, &bindings)
        .expect("POSIX script quoting keeps the marker inside one literal");

    // `output' file.txt'` is the measured `Sh` encoding of the spaced path.
    assert_eq!(actual, expected);
}

/// Resume unquoted interpolation after a here-string containing literal quotes.
#[test]
fn power_shell_scripts_track_here_string_terminators() {
    let template = format!("@'\nbody ' quote\n'@\nWrite-Output {OUTS_TOKEN}");
    let actual = interpolate_script_with_bindings(&template, &script_bindings())
        .expect("a closed here-string must not alter later marker context");

    assert_eq!(
        actual,
        "@'\nbody ' quote\n'@\nWrite-Output 'output file.txt'"
    );

    let same_line_template = format!("@'\nbody\n'@; Write-Output {OUTS_TOKEN}");
    let same_line_actual =
        interpolate_script_with_bindings(&same_line_template, &script_bindings())
            .expect("a here-string terminator may be followed by another command on its line");

    assert_eq!(
        same_line_actual,
        "@'\nbody\n'@; Write-Output 'output file.txt'"
    );
}

/// Keep typographic quotes in here-string data from changing later marker sites.
#[rstest]
#[case::single_quoted(format!("@'\nbody ’\n'@\nWrite-Output {INS_TOKEN}"), INS_TOKEN, "'; New-Item pwned; Write-Output x'")]
#[case::double_quoted(format!("@\"\nbody “\n\"@\nWrite-Output {OUTS_TOKEN}"), OUTS_TOKEN, "'output file.txt'")]
fn power_shell_scripts_keep_here_string_quotes_literal_after_terminator(
    #[case] template: String,
    #[case] marker: &str,
    #[case] encoded_path: &str,
) {
    let actual = interpolate_script_with_bindings(&template, &script_bindings())
        .expect("quotes inside a here-string are literal data");
    let expected = template.replace(marker, encoded_path);

    assert_eq!(actual, expected);
}

/// Reject markers after here-string or braced-variable syntax the scanner cannot model.
#[rstest]
#[case::here_string_inside_command_substitution(format!(
    "Write-Output $(@'\n'\n'@)') {OUTS_TOKEN} '"
))]
#[case::braced_variable_name(format!("Write-Output ${{a'b}} '{OUTS_TOKEN}'"))]
fn power_shell_scripts_reject_markers_after_unmodelled_syntax(#[case] template: String) {
    let bindings = CommandBindings::new(
        &[],
        &[Utf8PathBuf::from("; New-Item pwned; Write-Output x")],
        RecipeShell::PowerShell,
    );
    let error = interpolate_script_with_bindings(&template, &bindings)
        .expect_err("unmodelled PowerShell syntax must keep later markers protected");

    assert!(matches!(error, IrGenError::InvalidCommand { command, .. } if command == template));
}

/// Fail closed when a comment quote changes the shared PowerShell quote state.
#[test]
fn power_shell_script_comment_quotes_can_reject_later_markers() {
    let template = format!("# don't parse this comment\nWrite-Output {OUTS_TOKEN}");
    let error = interpolate_script_with_bindings(&template, &script_bindings())
        .expect_err("PowerShell script comments are not an inert scanner region");

    assert!(matches!(error, IrGenError::InvalidCommand { command, .. } if command == template));
}

/// Keep plain comment text from poisoning later unquoted marker sites.
#[test]
fn power_shell_scripts_interpolate_markers_after_plain_comments() {
    let template = format!("# ordinary comment\nWrite-Output {OUTS_TOKEN}");
    let actual = interpolate_script_with_bindings(&template, &script_bindings())
        .expect("plain comments do not change PowerShell quote state");

    assert_eq!(actual, "# ordinary comment\nWrite-Output 'output file.txt'");
}

/// Resume safe interpolation after PowerShell stop parsing ends.
#[rstest]
#[case::newline(format!("cmd /c --% literal\nWrite-Output {OUTS_TOKEN}"), "cmd /c --% literal\nWrite-Output 'output file.txt'")]
#[case::pipeline(format!("cmd /c --% echo \"prefix | text\" | Write-Output {OUTS_TOKEN}"), "cmd /c --% echo \"prefix | text\" | Write-Output 'output file.txt'")]
fn power_shell_scripts_resume_after_stop_parsing(#[case] template: String, #[case] expected: &str) {
    let actual = interpolate_script_with_bindings(&template, &script_bindings())
        .expect("stop parsing ends at a newline or pipeline");

    assert_eq!(actual, expected);
}

/// Do not treat `--%` embedded in an ordinary bare argument as a stop token.
#[rstest]
#[case::embedded_middle(format!("Write-Output prefix--%suffix {OUTS_TOKEN}"))]
#[case::embedded_start(format!("Write-Output --%suffix {OUTS_TOKEN}"))]
#[case::after_hash_in_bare_argument(format!("Write-Output prefix#--%suffix {OUTS_TOKEN}"))]
fn power_shell_scripts_accept_non_token_stop_parsing_text(#[case] template: String) {
    let actual = interpolate_script_with_bindings(&template, &script_bindings())
        .expect("only the exact standalone token starts stop parsing");

    assert_eq!(actual, template.replace(OUTS_TOKEN, "'output file.txt'"));
}

/// Reject markers whose quote context can be hidden by a multiline block comment.
#[rstest]
#[case::windows_semicolon_path("; New-Item pwned; Write-Output x")]
#[case::command_substitution_path("$(New-Item pwned)")]
fn power_shell_scripts_reject_markers_after_block_comment_openers(#[case] input_path: &str) {
    let template = format!("<#\n\"\n#>\nWrite-Output \"{INS_TOKEN}\"");
    let bindings = CommandBindings::new(
        &[Utf8PathBuf::from(input_path)],
        &[Utf8PathBuf::from("output file.txt")],
        RecipeShell::PowerShell,
    );
    let error = interpolate_script_with_bindings(&template, &bindings)
        .expect_err("a block comment can hide the real PowerShell quote context");

    assert!(matches!(error, IrGenError::InvalidCommand { command, .. } if command == template));
}

/// Accept unquoted markers and encode the semicolon path as one PowerShell word.
#[test]
fn power_shell_scripts_quote_unquoted_paths_in_multiline_text() {
    let template = format!("Copy-Item {INS_TOKEN} {OUTS_TOKEN}\nWrite-Output done");
    let actual = interpolate_script_with_bindings(&template, &script_bindings())
        .expect("unquoted PowerShell script markers should interpolate");

    assert_eq!(
        actual,
        "Copy-Item '; New-Item pwned; Write-Output x' 'output file.txt'\nWrite-Output done"
    );
}
