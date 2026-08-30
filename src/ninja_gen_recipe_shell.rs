//! Renders legacy recipes for the interpreter selected by the host contract.

use base64::{Engine as _, engine::general_purpose::STANDARD};

use super::NinjaGenError;
use super::ninja_gen_escape::{NinjaValue, ShellText, escape_ninja_value};
use crate::recipe_shell::RecipeShell;

/// Leave space for Windows' terminating command-line NUL character.
const MAX_POWER_SHELL_COMMAND_LINE: usize = 32_766;

/// Prefix shared by every encoded Windows PowerShell recipe invocation.
const POWER_SHELL_COMMAND_PREFIX: &str =
    "powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand ";

/// Fixed command that executes Ninja's oversized-recipe response file.
///
/// Ninja expands `$rspfile` into a separately quoted `-File` argument, so the
/// path never becomes PowerShell source text.
const POWER_SHELL_RESPONSE_FILE_COMMAND: &str =
    "powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"$rspfile\"";

/// Start the one-line response-file bootstrap before its Base64 payload.
const POWER_SHELL_RESPONSE_FILE_SCRIPT_PREFIX: &str = "$netsukePayload = '";

/// Finish the one-line response-file bootstrap after its Base64 payload.
const POWER_SHELL_RESPONSE_FILE_SCRIPT_SUFFIX: &str = concat!(
    "'; $netsukeScript = try { [Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($netsukePayload)) } ",
    "catch { throw \"Netsuke could not decode the PowerShell response file: $($_.Exception.Message)\" }; ",
    ". ([ScriptBlock]::Create($netsukeScript)); if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }"
);

/// Represent one Ninja command binding and its optional response-file payload.
pub(super) enum RenderedRecipeCommand {
    /// Run the command directly from the Ninja command binding.
    Direct(NinjaValue),
    /// Let Ninja materialize and remove a response file for one build edge.
    ResponseFile {
        /// Fixed command that starts the response file as a PowerShell script.
        command: NinjaValue,
        /// One-line bootstrap script that contains Base64 UTF-16LE recipe text.
        content: NinjaValue,
    },
}

impl RenderedRecipeCommand {
    /// Borrow the command text written into the Ninja rule.
    pub(super) const fn command(&self) -> &NinjaValue {
        match self {
            Self::Direct(command) | Self::ResponseFile { command, .. } => command,
        }
    }

    /// Borrow the optional response-file content written by Ninja at execution time.
    pub(super) const fn response_file_content(&self) -> Option<&NinjaValue> {
        match self {
            Self::Direct(_) => None,
            Self::ResponseFile { content, .. } => Some(content),
        }
    }
}

impl RecipeShell {
    /// Render completed recipe text as a safe Ninja command binding value.
    pub(super) fn command_value(
        self,
        script: &ShellText,
    ) -> Result<RenderedRecipeCommand, NinjaGenError> {
        match self {
            Self::Posix => escape_ninja_value(script).map(RenderedRecipeCommand::Direct),
            Self::PowerShell => {
                let power_shell_script = Self::power_shell_script(script);
                Self::power_shell_command(&power_shell_script)
            }
            Self::Bash => Self::bash_command(script).map(RenderedRecipeCommand::Direct),
        }
    }

    /// Build one shared-scope PowerShell script for an ordered command list.
    pub(crate) fn command_list_script(self, entries: &[String]) -> Option<String> {
        if self != Self::PowerShell {
            return None;
        }
        let mut script = String::new();
        for entry in entries {
            script.push_str("$LASTEXITCODE = 0\n");
            script.push_str(entry);
            script.push_str("\nif ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\n");
        }
        Some(script)
    }

    /// Add PowerShell's terminating-error and native-process exit policy.
    fn power_shell_script(script: &ShellText) -> ShellText {
        ShellText::new(format!(
            "$ErrorActionPreference = 'Stop'\n$LASTEXITCODE = 0\n{}\nif ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}",
            script.as_str()
        ))
    }

    /// Encode one PowerShell script without exposing its text to Ninja parsing.
    fn power_shell_command(script: &ShellText) -> Result<RenderedRecipeCommand, NinjaGenError> {
        if script.as_str().contains('\0') {
            return Err(NinjaGenError::UnsafeNinjaValue);
        }
        let utf16le = script
            .as_str()
            .encode_utf16()
            .flat_map(|code_unit| {
                #[expect(
                    clippy::little_endian_bytes,
                    reason = "PowerShell -EncodedCommand requires UTF-16LE input"
                )]
                let bytes = code_unit.to_le_bytes();
                bytes
            })
            .collect::<Vec<_>>();
        let command = STANDARD.encode(utf16le);
        let length = POWER_SHELL_COMMAND_PREFIX.len() + command.len();
        if length > MAX_POWER_SHELL_COMMAND_LINE {
            return Ok(RenderedRecipeCommand::ResponseFile {
                command: NinjaValue::from_encoded(POWER_SHELL_RESPONSE_FILE_COMMAND.into()),
                content: power_shell_response_file_content(&command)?,
            });
        }
        Ok(RenderedRecipeCommand::Direct(NinjaValue::from_encoded(
            format!("{POWER_SHELL_COMMAND_PREFIX}{command}"),
        )))
    }

    /// Wrap one POSIX recipe in the explicit Windows Bash compatibility runtime.
    fn bash_command(script: &ShellText) -> Result<NinjaValue, NinjaGenError> {
        let command = format!("bash.exe -e -c {}", windows_argument(script.as_str()));
        escape_ninja_value(&ShellText::new(command))
    }
}

/// Build the one-line PowerShell bootstrap stored in Ninja's response file.
///
/// The response file is itself a `-File` script so Windows PowerShell receives
/// its path through normal argument parsing. The payload remains Base64 because
/// Ninja bindings cannot safely carry a multi-line legacy recipe directly.
fn power_shell_response_file_content(encoded_script: &str) -> Result<NinjaValue, NinjaGenError> {
    let bootstrap = format!(
        "{POWER_SHELL_RESPONSE_FILE_SCRIPT_PREFIX}{encoded_script}{POWER_SHELL_RESPONSE_FILE_SCRIPT_SUFFIX}"
    );
    escape_ninja_value(&ShellText::new(bootstrap))
}

/// Quote one argument according to the Windows `CommandLineToArgvW` convention.
fn windows_argument(argument: &str) -> String {
    let mut quoted = String::with_capacity(argument.len() + 2);
    quoted.push('"');
    let mut backslashes = 0usize;
    for character in argument.chars() {
        if character == '\\' {
            backslashes += 1;
        } else if character == '"' {
            quoted.push_str(&"\\".repeat(backslashes.saturating_mul(2).saturating_add(1)));
            quoted.push('"');
            backslashes = 0;
        } else {
            quoted.push_str(&"\\".repeat(backslashes));
            quoted.push(character);
            backslashes = 0;
        }
    }
    quoted.push_str(&"\\".repeat(backslashes.saturating_mul(2)));
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    //! Verifies interpreter-specific Ninja command rendering.

    use super::{
        POWER_SHELL_COMMAND_PREFIX, POWER_SHELL_RESPONSE_FILE_COMMAND, RecipeShell,
        RenderedRecipeCommand, windows_argument,
    };
    use crate::ninja_gen::ninja_gen_escape::ShellText;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
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
}
