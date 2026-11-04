//! Renders legacy recipes for the interpreter selected by the host contract.

use base64::{Engine as _, engine::general_purpose::STANDARD};

use super::NinjaGenError;
use super::escape::{NinjaValue, ShellText, escape_ninja_value};
use crate::recipe_shell::{BashExecutable, RecipeShell};

mod transport;

pub(super) use transport::escape_posix_script;

/// Leave space for Windows' terminating command-line NUL character.
const MAX_POWER_SHELL_COMMAND_LINE: usize = 32_766;

/// Bound the complete PowerShell parent script before UTF-16LE and Base64 encoding.
const MAX_POWER_SHELL_RECIPE_BYTES: usize = 1024 * 1024;

/// Prefix shared by every encoded Windows PowerShell recipe invocation.
const POWER_SHELL_COMMAND_PREFIX: &str =
    "powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand ";

/// Executable expression for one fresh PowerShell process used by an isolated entry.
const ISOLATED_POWER_SHELL_CHILD_EXECUTABLE: &str = "\"$PSHOME\\powershell.exe\"";

/// Arguments shared by every encoded Windows PowerShell child entry.
const ISOLATED_POWER_SHELL_CHILD_ARGUMENT_PREFIX: &str = concat!(
    "-NoLogo -NoProfile -NonInteractive ",
    "-ExecutionPolicy Bypass -EncodedCommand "
);

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
    "try { . ([ScriptBlock]::Create($netsukeScript)) } finally { Remove-Item -LiteralPath $PSCommandPath -Force }; ",
    "if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }"
);

/// Select how an isolated PowerShell process treats native stderr.
#[derive(Clone, Copy)]
enum PowerShellErrorAction {
    /// Stop on PowerShell non-terminating errors.
    Stop,
    /// Pass native stderr through while cmdlet defaults and exit checks fail fast.
    ContinueNativeStderr,
}

impl PowerShellErrorAction {
    /// Return the preference assigned before the generated recipe runs.
    const fn preference(self) -> &'static str {
        match self {
            Self::Stop => "Stop",
            Self::ContinueNativeStderr => "Continue",
        }
    }
}

/// Represent one Ninja command binding and its optional response-file payload.
pub(super) enum RenderedRecipeCommand {
    /// Run the command directly from the Ninja command binding.
    Direct(NinjaValue),
    /// Let Ninja materialize a response file for one build edge.
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
    #[cfg(test)]
    pub(super) fn command_value(
        self,
        script: &ShellText,
    ) -> Result<RenderedRecipeCommand, NinjaGenError> {
        self.command_value_with_bash(script, None)
    }

    /// Render completed recipe text with an optional runner-selected Bash path.
    ///
    /// # Errors
    ///
    /// Returns [`NinjaGenError`] when the selected renderer rejects the recipe
    /// text or its command transport exceeds a supported limit.
    pub(super) fn command_value_with_bash(
        self,
        script: &ShellText,
        bash_executable: Option<&BashExecutable>,
    ) -> Result<RenderedRecipeCommand, NinjaGenError> {
        self.command_value_with_error_action(script, PowerShellErrorAction::Stop, bash_executable)
    }

    /// Render a recipe with the error policy required by isolated PowerShell lists.
    pub(super) fn isolated_command_list_value(
        self,
        script: &ShellText,
    ) -> Result<RenderedRecipeCommand, NinjaGenError> {
        self.command_value_with_error_action(
            script,
            PowerShellErrorAction::ContinueNativeStderr,
            None,
        )
    }

    /// Render a command after selecting its PowerShell error policy.
    fn command_value_with_error_action(
        self,
        script: &ShellText,
        error_action: PowerShellErrorAction,
        bash_executable: Option<&BashExecutable>,
    ) -> Result<RenderedRecipeCommand, NinjaGenError> {
        match self {
            Self::Posix => escape_ninja_value(script).map(RenderedRecipeCommand::Direct),
            Self::PowerShell => {
                let power_shell_script = match error_action {
                    PowerShellErrorAction::Stop => Self::power_shell_script(script),
                    PowerShellErrorAction::ContinueNativeStderr => {
                        Self::power_shell_script_with_error_action(script, error_action)
                    }
                };
                Self::validate_power_shell_recipe_size(&power_shell_script)?;
                Self::power_shell_command(&power_shell_script)
            }
            Self::Bash => {
                Self::bash_command(script, bash_executable).map(RenderedRecipeCommand::Direct)
            }
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

    /// Build a PowerShell parent script that gives each entry a fresh process.
    pub(crate) fn isolated_command_list_script(
        self,
        entries: &[String],
    ) -> Result<Option<String>, NinjaGenError> {
        if self != Self::PowerShell {
            return Ok(None);
        }
        let mut script = String::new();
        for (entry_index, entry) in entries.iter().enumerate() {
            let child_script = Self::power_shell_script_for_entry(entry);
            if child_script.contains('\0') {
                return Err(NinjaGenError::UnsafeNinjaValue);
            }
            let payload = encode_power_shell_script(&child_script);
            let argument_characters = Self::isolated_child_argument_characters(&payload);
            Self::validate_isolated_child_command_line(entry_index + 1, argument_characters)?;
            Self::push_isolated_child_runtime_length_guard(
                &mut script,
                entry_index + 1,
                argument_characters,
            );
            script.push_str("$LASTEXITCODE = 0\n& $netsukeChildExecutable ");
            script.push_str(ISOLATED_POWER_SHELL_CHILD_ARGUMENT_PREFIX);
            script.push_str(&payload);
            script.push_str("\nif ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\n");
        }
        Ok(Some(script))
    }

    /// Measure the fixed arguments passed to one isolated PowerShell child.
    fn isolated_child_argument_characters(payload: &str) -> usize {
        ISOLATED_POWER_SHELL_CHILD_ARGUMENT_PREFIX
            .encode_utf16()
            .count()
            .saturating_add(payload.encode_utf16().count())
    }

    /// Reject an isolated child command that exceeds Windows' command-line limit.
    fn validate_isolated_child_command_line(
        entry_index: usize,
        argument_characters: usize,
    ) -> Result<(), NinjaGenError> {
        let actual_characters = ISOLATED_POWER_SHELL_CHILD_EXECUTABLE
            .encode_utf16()
            .count()
            .saturating_add(1)
            .saturating_add(argument_characters);
        if actual_characters > MAX_POWER_SHELL_COMMAND_LINE {
            return Err(NinjaGenError::PowerShellCommandLineTooLong {
                entry_index,
                actual_characters,
                maximum_characters: MAX_POWER_SHELL_COMMAND_LINE,
            });
        }
        Ok(())
    }

    /// Append a runtime guard for the expanded PowerShell executable path.
    ///
    /// The generated parent cannot know `$PSHOME` until PowerShell evaluates it.
    fn push_isolated_child_runtime_length_guard(
        script: &mut String,
        entry_index: usize,
        argument_characters: usize,
    ) {
        script.push_str("$netsukeChildExecutable = \"$PSHOME\\powershell.exe\"\n");
        script
            .push_str("$netsukeChildCommandLineLength = 2 + $netsukeChildExecutable.Length + 1 + ");
        script.push_str(&argument_characters.to_string());
        script.push('\n');
        script.push_str("if ($netsukeChildCommandLineLength -gt ");
        script.push_str(&MAX_POWER_SHELL_COMMAND_LINE.to_string());
        script.push_str(
            ") {\n    [Console]::Error.WriteLine('Netsuke: isolated PowerShell command-list entry ",
        );
        script.push_str(&entry_index.to_string());
        script.push_str(" exceeds the ");
        script.push_str(&MAX_POWER_SHELL_COMMAND_LINE.to_string());
        script.push_str(
            "-character child command-line limit (' + $netsukeChildCommandLineLength + ' characters).')\n    exit 1\n}\n",
        );
    }

    /// Keep native stderr informational and cmdlet errors fail-fast for one entry.
    fn power_shell_script_for_entry(entry: &str) -> String {
        format!(
            "$ErrorActionPreference = 'Continue'\n$PSDefaultParameterValues['*:ErrorAction'] = 'Stop'\n$LASTEXITCODE = 0\n{entry}\nif ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}"
        )
    }

    /// Add PowerShell's terminating-error and native-process exit policy.
    fn power_shell_script(script: &ShellText) -> ShellText {
        Self::power_shell_script_with_error_action(script, PowerShellErrorAction::Stop)
    }

    /// Apply the chosen PowerShell error and native-process exit policies.
    fn power_shell_script_with_error_action(
        script: &ShellText,
        error_action: PowerShellErrorAction,
    ) -> ShellText {
        ShellText::new(format!(
            "$ErrorActionPreference = '{}'\n$LASTEXITCODE = 0\n{}\nif ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}",
            error_action.preference(),
            script.as_str()
        ))
    }

    /// Reject recipe text that would exceed the bounded in-memory encoder budget.
    fn validate_power_shell_recipe_size(script: &ShellText) -> Result<(), NinjaGenError> {
        let actual_bytes = script.as_str().len();
        if actual_bytes > MAX_POWER_SHELL_RECIPE_BYTES {
            return Err(NinjaGenError::PowerShellRecipeTooLarge {
                actual_bytes,
                maximum_bytes: MAX_POWER_SHELL_RECIPE_BYTES,
            });
        }
        Ok(())
    }

    /// Encode one PowerShell script without exposing its text to Ninja parsing.
    fn power_shell_command(script: &ShellText) -> Result<RenderedRecipeCommand, NinjaGenError> {
        if script.as_str().contains('\0') {
            return Err(NinjaGenError::UnsafeNinjaValue);
        }
        let command = encode_power_shell_script(script.as_str());
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
    fn bash_command(
        script: &ShellText,
        bash_executable: Option<&BashExecutable>,
    ) -> Result<NinjaValue, NinjaGenError> {
        let executable = bash_executable.map_or_else(
            || "bash.exe".to_owned(),
            |path| windows_argument(path.as_path().as_str()),
        );
        let command = format!("{executable} -e -c {}", windows_argument(script.as_str()));
        escape_ninja_value(&ShellText::new(command))
    }
}

/// Encode PowerShell source as the UTF-16LE Base64 accepted by `-EncodedCommand`.
fn encode_power_shell_script(script: &str) -> String {
    let utf16le = script
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
    STANDARD.encode(utf16le)
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
#[path = "../shell_tests.rs"]
mod tests;
