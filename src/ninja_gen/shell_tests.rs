//! Verifies interpreter-specific Ninja command rendering.

use super::{
    ISOLATED_POWER_SHELL_CHILD_ARGUMENT_PREFIX, ISOLATED_POWER_SHELL_CHILD_EXECUTABLE,
    MAX_POWER_SHELL_COMMAND_LINE, MAX_POWER_SHELL_RECIPE_BYTES, POWER_SHELL_COMMAND_PREFIX,
    POWER_SHELL_RESPONSE_FILE_COMMAND, RecipeShell, RenderedRecipeCommand,
    encode_power_shell_script, windows_argument,
};
use crate::ninja_gen::escape::ShellText;
use crate::recipe_shell::BashExecutable;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use camino::Utf8PathBuf;
use proptest::prelude::*;

/// Verify that encoded PowerShell commands hide recipe dollars from Ninja.
#[test]
fn power_shell_command_hides_recipe_dollars_from_ninja() {
    let rendered = RecipeShell::PowerShell
        .command_value(&ShellText::new("$env:NETSUKE_SMOKE".into()))
        .expect("PowerShell encoding should succeed")
        .command()
        .to_string();
    assert!(rendered.starts_with("powershell.exe "));
    assert!(!rendered.contains("NETSUKE_SMOKE"));
}

/// Verify that Windows argument quoting preserves quotes and trailing slashes.
#[test]
fn windows_argument_preserves_quotes_and_trailing_backslashes() {
    assert_eq!(
        windows_argument("a \\\"b\\\"\\\\"),
        "\"a \\\\\\\"b\\\\\\\"\\\\\\\\\""
    );
}

/// Verify that the Bash compatibility renderer invokes `bash.exe -e -c`.
#[test]
fn bash_command_value_uses_the_explicit_windows_compatibility_runtime() {
    let rendered = RecipeShell::Bash
        .command_value(&ShellText::new("echo \"hello world\"\\".into()))
        .expect("Bash command rendering should succeed")
        .command()
        .to_string();
    assert_eq!(rendered, "bash.exe -e -c \"echo \\\"hello world\\\"\\\\\"");
}

/// Bind a selected Bash path and apply Windows quoting before Ninja escaping.
#[test]
fn bash_command_value_uses_the_preflighted_executable_path() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let executable_path = Utf8PathBuf::from_path_buf(
        workspace
            .path()
            .join("Git Bash λ $")
            .join("bin")
            .join("bash.exe"),
    )
    .map_err(|_| anyhow::anyhow!("fixture executable path should be UTF-8"))?;
    let executable = BashExecutable::from_absolute_path(executable_path)?;

    let rendered = RecipeShell::Bash
        .command_value_with_bash(&ShellText::new("echo ready".into()), Some(&executable))?
        .command()
        .to_string();

    let expected = format!(
        "{} -e -c \"echo ready\"",
        windows_argument(executable.as_path().as_str()).replace('$', "$$")
    );
    anyhow::ensure!(
        rendered == expected,
        "unexpected preflighted Bash command: {rendered}"
    );
    Ok(())
}

/// Verify that a PowerShell list checks native failure before its next entry.
#[test]
fn power_shell_lists_check_the_captured_status_before_the_next_entry() {
    let script = RecipeShell::PowerShell
        .command_list_script(&["cmd.exe /c exit 7".into(), "Write-Output later".into()])
        .expect("PowerShell should render command-list scripts");
    let failure_check = script
        .find("if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }")
        .expect("first command should have an immediate status check");
    let next_entry = script
        .find("Write-Output later")
        .expect("second command should be rendered");
    assert!(failure_check < next_entry);
}

/// Verify that PowerShell list bookkeeping does not reserve a user variable.
#[test]
fn power_shell_lists_do_not_reserve_user_variable_names() {
    let script = RecipeShell::PowerShell
        .command_list_script(&[
            "$netsuke_exit_code = 'keep'".into(),
            "if ($netsuke_exit_code -ne 'keep') { throw 'lost' }".into(),
        ])
        .expect("PowerShell should render command-list scripts");
    assert!(!script.contains("$netsuke_exit_code = $LASTEXITCODE"));
}

/// Verify isolated entries invoke a new child with the existing error policy.
#[test]
fn isolated_power_shell_entries_use_encoded_child_processes() {
    let script = isolated_power_shell_script();
    assert!(
        script
            .contains("-NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand")
    );
    assert_eq!(
        script
            .matches("if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }")
            .count(),
        2
    );

    let payload = script
        .lines()
        .find(|line| line.starts_with("& $netsukeChildExecutable "))
        .and_then(|line| line.split("-EncodedCommand ").nth(1))
        .expect("first child invocation should include a payload");
    let decoded = STANDARD
        .decode(payload)
        .expect("child payload should be Base64");
    let (pairs, remainder) = decoded.as_chunks::<2>();
    assert!(
        remainder.is_empty(),
        "UTF-16LE payload has an odd byte count"
    );
    let units = pairs
        .iter()
        .map(|[low, high]| u16::from(*low) | (u16::from(*high) << 8))
        .collect::<Vec<_>>();
    let child = String::from_utf16(&units).expect("child payload should be UTF-16LE");
    assert!(child.starts_with(
        "$ErrorActionPreference = 'Continue'\n$PSDefaultParameterValues['*:ErrorAction'] = 'Stop'\n$LASTEXITCODE = 0\n"
    ));
    assert!(child.contains("$env:VALUE = 'first'"));
    assert!(child.ends_with("if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }"));
}

/// Verify child launch checks account for the expanded executable path.
#[test]
fn isolated_power_shell_child_size_checks_include_the_runtime_path() {
    let script = isolated_power_shell_script();
    assert_eq!(
        script
            .matches("$netsukeChildExecutable = \"$PSHOME\\powershell.exe\"")
            .count(),
        2
    );
    assert_eq!(
        script
            .matches("if ($netsukeChildCommandLineLength -gt 32766)")
            .count(),
        2
    );
    assert_eq!(
        script
            .matches("& $netsukeChildExecutable -NoLogo -NoProfile -NonInteractive")
            .count(),
        2
    );
}

/// Render a representative isolated PowerShell command list for renderer tests.
fn isolated_power_shell_script() -> String {
    RecipeShell::PowerShell
        .isolated_command_list_script(&["$env:VALUE = 'first'".into(), "Write-Output later".into()])
        .expect("isolated PowerShell list should render")
        .expect("PowerShell should provide an isolated renderer")
}

/// Reject a child payload whose encoded command line exceeds Windows' limit.
#[test]
fn isolated_power_shell_entries_report_the_child_command_line_size() {
    let entry = "x".repeat(13_000);
    let result = RecipeShell::PowerShell.isolated_command_list_script(&[entry]);
    assert!(matches!(
        result,
        Err(super::NinjaGenError::PowerShellCommandLineTooLong {
            entry_index: 1,
            actual_characters,
            maximum_characters: MAX_POWER_SHELL_COMMAND_LINE,
        }) if actual_characters > MAX_POWER_SHELL_COMMAND_LINE
    ));
}

/// Leave non-PowerShell routes to their shell-specific list renderer.
#[test]
fn isolated_power_shell_builder_is_unavailable_for_other_shells() {
    assert_eq!(
        RecipeShell::Posix
            .isolated_command_list_script(&["echo hi".into()])
            .expect("other shell routes need no PowerShell errors"),
        None
    );
}

/// Verify that oversized recipes use a response file instead of a command-line payload.
#[test]
fn power_shell_commands_exceeding_the_windows_limit_use_a_response_file() {
    let recipe = "x".repeat(12_500);
    let result = RecipeShell::PowerShell
        .command_value(&ShellText::new(recipe.clone()))
        .expect("oversized recipes should use a response file");
    assert!(matches!(
        &result,
        RenderedRecipeCommand::ResponseFile { .. }
    ));
    assert!(!result.command().to_string().contains(&recipe));
    assert_eq!(
        result.command().to_string(),
        POWER_SHELL_RESPONSE_FILE_COMMAND
    );
    let content = result
        .response_file_content()
        .expect("oversized recipes should include a response-file script")
        .to_string();
    assert!(!content.contains(&recipe));
    assert!(content.contains("$$netsukePayload"));
}

/// Reject NUL-bearing PowerShell text before either transport can serialize it.
#[test]
fn power_shell_commands_reject_unsafe_ninja_control_characters() {
    let result = RecipeShell::PowerShell.command_value(&ShellText::new("safe\0unsafe".into()));
    assert!(matches!(
        result,
        Err(super::NinjaGenError::UnsafeNinjaValue)
    ));
}

/// Reject recipe text that would exceed the bounded PowerShell encoder allocation.
#[test]
fn power_shell_commands_reject_recipes_larger_than_the_encoding_limit() {
    let recipe = "x".repeat(MAX_POWER_SHELL_RECIPE_BYTES + 1);
    let result = RecipeShell::PowerShell.command_value(&ShellText::new(recipe));
    assert!(matches!(
        result,
        Err(super::NinjaGenError::PowerShellRecipeTooLarge {
            actual_bytes,
            maximum_bytes: MAX_POWER_SHELL_RECIPE_BYTES,
        }) if actual_bytes > MAX_POWER_SHELL_RECIPE_BYTES
    ));
}

proptest! {
    /// Verify scalar PowerShell encoding round-trips Unicode and dollar expressions.
    #[test]
    fn power_shell_scalar_encoding_round_trips(
        text in proptest::collection::vec(any::<char>().prop_filter("no NUL", |character| *character != '\0'), 0..96),
    ) {
        let ordinary = text.into_iter().collect::<String>();
        let recipe = format!("{ordinary} $env:NAME $value ${{name}}");
        let rendered = RecipeShell::PowerShell
            .command_value(&ShellText::new(recipe.clone()))
            .expect("bounded PowerShell recipe should encode")
            .command()
            .to_string();
        let payload = rendered
            .strip_prefix(POWER_SHELL_COMMAND_PREFIX)
            .expect("rendered command should carry an encoded payload");
        let decoded_bytes = STANDARD.decode(payload).expect("payload should be Base64");
        let (pairs, remainder) = decoded_bytes.as_chunks::<2>();
        prop_assert!(remainder.is_empty());
        let code_units = pairs
            .iter()
            .map(|[low, high]| u16::from(*low) | (u16::from(*high) << 8))
            .collect::<Vec<_>>();
        let decoded = String::from_utf16(&code_units).expect("payload should be UTF-16LE");
        let expected = RecipeShell::power_shell_script(&ShellText::new(recipe.clone()));
        prop_assert_eq!(decoded, expected.as_str());
        prop_assert!(!rendered.contains(&recipe));
        prop_assert!(!rendered.contains("$env:NAME"));
    }

    /// Verify that every generated PowerShell list entry has an immediate failure check.
    #[test]
    fn power_shell_lists_check_each_entry_before_the_next(
        labels in proptest::collection::vec("[a-z]{1,12}", 1..8),
    ) {
        let entries = labels
            .iter()
            .map(|label| format!("Write-Output {label}"))
            .collect::<Vec<_>>();
        let script = RecipeShell::PowerShell
            .command_list_script(&entries)
            .expect("PowerShell should render command-list scripts");
        let check = "if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }";
        let expected = entries.iter().fold(String::new(), |mut expected_script, entry| {
            expected_script.push_str("$LASTEXITCODE = 0\n");
            expected_script.push_str(entry);
            expected_script.push('\n');
            expected_script.push_str(check);
            expected_script.push('\n');
            expected_script
        });
        prop_assert_eq!(script, expected);
    }
}

/// Verify the isolated child prefix and encoded payload use UTF-16 characters.
#[test]
fn isolated_child_prefix_is_the_length_accounting_prefix() {
    let child_script = RecipeShell::power_shell_script_for_entry("Write-Output hi");
    let payload = encode_power_shell_script(&child_script);
    let expected_length = ISOLATED_POWER_SHELL_CHILD_EXECUTABLE.encode_utf16().count()
        + 1
        + ISOLATED_POWER_SHELL_CHILD_ARGUMENT_PREFIX
            .encode_utf16()
            .count()
        + payload.encode_utf16().count();
    let script = RecipeShell::PowerShell
        .isolated_command_list_script(&["Write-Output hi".into()])
        .expect("isolated list should render")
        .expect("PowerShell route should be available");
    assert!(script.contains(&payload));
    assert!(expected_length < MAX_POWER_SHELL_COMMAND_LINE);
}
