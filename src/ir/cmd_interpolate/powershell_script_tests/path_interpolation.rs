//! Test PowerShell path interpolation and POSIX contrast cases.

use super::posix_output_case;
use super::*;

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
    posix_output_case!(format!("Write-Output '{INS_TOKEN}' > {OUTS_TOKEN}"),
    "Write-Output '; New-Item pwned; Write-Output x' > output' file.txt'",
))]
#[case::single_quoted_output(
    posix_output_case!(format!("Write-Output {INS_TOKEN} > '{OUTS_TOKEN}'"),
    "Write-Output '; New-Item pwned; Write-Output x' > 'output file.txt'",
))]
#[case::double_quoted_input(
    posix_output_case!(format!("Write-Output \"{INS_TOKEN}\" > {OUTS_TOKEN}"),
    "Write-Output \"; New-Item pwned; Write-Output x\" > output' file.txt'",
))]
#[case::double_quoted_output(
    posix_output_case!(format!("Write-Output {INS_TOKEN} > \"{OUTS_TOKEN}\""),
    "Write-Output '; New-Item pwned; Write-Output x' > \"output file.txt\"",
))]
fn posix_scripts_still_interpolate_quoted_markers(#[case] scenario: ScriptScenario) {
    // `output' file.txt'` is the measured `Sh` encoding of the spaced path.
    scenario.verify();
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

    let assignment_template = format!("$value=@'\nbody\n'@\nWrite-Output {OUTS_TOKEN}");
    let assignment_actual =
        interpolate_script_with_bindings(&assignment_template, &script_bindings())
            .expect("an assignment operator is a valid here-string boundary");

    assert_eq!(
        assignment_actual,
        "$value=@'\nbody\n'@\nWrite-Output 'output file.txt'"
    );
}

/// Keep typographic quotes in here-string data from changing later marker sites.
#[rstest]
#[case::single_quoted(HereStringQuoteCase {
    template: format!("@'\nbody ’\n'@\nWrite-Output {INS_TOKEN}"),
    marker: INS_TOKEN,
    encoded_path: "'; New-Item pwned; Write-Output x'",
})]
#[case::double_quoted(HereStringQuoteCase {
    template: format!("@\"\nbody “\n\"@\nWrite-Output {OUTS_TOKEN}"),
    marker: OUTS_TOKEN,
    encoded_path: "'output file.txt'",
})]
fn power_shell_scripts_keep_here_string_quotes_literal_after_terminator(
    #[case] scenario: HereStringQuoteCase,
) {
    let actual = interpolate_script_with_bindings(&scenario.template, &script_bindings())
        .expect("quotes inside a here-string are literal data");
    let expected = scenario
        .template
        .replace(scenario.marker, scenario.encoded_path);

    assert_eq!(actual, expected);
}
