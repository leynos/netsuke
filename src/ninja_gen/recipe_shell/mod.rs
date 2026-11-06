//! Renders legacy recipes for the interpreter selected by the host contract.

mod transport;

pub(super) use self::transport::{RenderedRecipeCommand, escape_posix_script};
use self::transport::{
    command_line_characters, encode_power_shell_script, power_shell_response_file_content,
    windows_argument,
};

use super::NinjaGenError;
use super::escape::{NinjaValue, ShellText, escape_ninja_value};
use crate::recipe_shell::{InterpreterExecutable, RecipeShell};

/// Leave space for Windows' terminating command-line NUL character.
const MAX_POWER_SHELL_COMMAND_LINE: usize = 32_766;

/// Bound the complete PowerShell parent script before UTF-16LE and Base64 encoding.
const MAX_POWER_SHELL_RECIPE_BYTES: usize = 1024 * 1024;

/// Logical executable name used by inspection-only PowerShell rendering.
const INSPECTION_POWER_SHELL_EXECUTABLE: &str = "powershell.exe";

/// Arguments shared by every encoded Windows PowerShell recipe invocation.
const POWER_SHELL_ENCODED_ARGUMENTS: &str = concat!(
    " -NoLogo -NoProfile -NonInteractive ",
    "-ExecutionPolicy Bypass -EncodedCommand "
);

/// Executable expression for one fresh PowerShell process used by an isolated entry.
const ISOLATED_POWER_SHELL_CHILD_EXECUTABLE: &str = "\"$PSHOME\\powershell.exe\"";

/// Arguments shared by every encoded Windows PowerShell child entry.
const ISOLATED_POWER_SHELL_CHILD_ARGUMENT_PREFIX: &str = concat!(
    "-NoLogo -NoProfile -NonInteractive ",
    "-ExecutionPolicy Bypass -EncodedCommand "
);

/// Arguments that execute Ninja's oversized-recipe response file.
///
/// Ninja expands `$rspfile` into a separately quoted `-File` argument, so the
/// path never becomes PowerShell source text.
const POWER_SHELL_RESPONSE_FILE_ARGUMENTS: &str =
    " -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"$rspfile\"";

/// Select whether rendered commands carry a resolved interpreter executable.
///
/// A [`InterpreterBinding::Bound`] binding is the only mode that can produce a
/// plan fit to execute: it renders the trusted absolute path instead of a
/// logical name that Windows could resolve through its executable search order.
#[derive(Clone, Copy)]
pub(crate) enum InterpreterBinding<'a> {
    /// Render generated commands against this trusted absolute executable.
    Bound(&'a InterpreterExecutable),
    /// Render logical interpreter names for inspection without executing them.
    ///
    /// No production caller selects this mode: every executing path binds an
    /// absolute executable. It exists for renderer tests and future
    /// inspection-only callers, so the library build has nothing to construct.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "inspection-only rendering is exercised by renderer tests"
        )
    )]
    InspectionOnly,
    /// Refuse to render an interpreter command that was not resolved.
    Unbound,
}

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

impl RecipeShell {
    /// Render completed recipe text for inspection without an executable binding.
    #[cfg(test)]
    pub(super) fn command_value(
        self,
        script: &ShellText,
    ) -> Result<RenderedRecipeCommand, NinjaGenError> {
        self.command_value_with_interpreter(script, InterpreterBinding::InspectionOnly)
    }

    /// Render completed recipe text with one interpreter executable binding.
    ///
    /// # Errors
    ///
    /// Returns [`NinjaGenError`] when the selected renderer rejects the recipe
    /// text or its command transport exceeds a supported limit, and
    /// [`NinjaGenError::InterpreterBindingRequired`] when the interpreter has no
    /// binding.
    pub(super) fn command_value_with_interpreter(
        self,
        script: &ShellText,
        binding: InterpreterBinding<'_>,
    ) -> Result<RenderedRecipeCommand, NinjaGenError> {
        self.command_value_with_error_action(script, PowerShellErrorAction::Stop, binding)
    }

    /// Render a recipe with the error policy required by isolated PowerShell lists.
    ///
    /// # Errors
    ///
    /// Returns [`NinjaGenError`] when the parent command cannot be rendered or
    /// the interpreter has no binding.
    pub(super) fn isolated_command_list_value(
        self,
        script: &ShellText,
        binding: InterpreterBinding<'_>,
    ) -> Result<RenderedRecipeCommand, NinjaGenError> {
        self.command_value_with_error_action(
            script,
            PowerShellErrorAction::ContinueNativeStderr,
            binding,
        )
    }

    /// Render a command after selecting its PowerShell error policy.
    fn command_value_with_error_action(
        self,
        script: &ShellText,
        error_action: PowerShellErrorAction,
        binding: InterpreterBinding<'_>,
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
                Self::power_shell_command(&power_shell_script, binding)
            }
            Self::Bash => Self::bash_command(script, binding).map(RenderedRecipeCommand::Direct),
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
    fn power_shell_command(
        script: &ShellText,
        binding: InterpreterBinding<'_>,
    ) -> Result<RenderedRecipeCommand, NinjaGenError> {
        if script.as_str().contains('\0') {
            return Err(NinjaGenError::UnsafeNinjaValue);
        }
        let executable = Self::power_shell_executable(binding)?;
        let command = encode_power_shell_script(script.as_str());
        let prefix = format!("{executable}{POWER_SHELL_ENCODED_ARGUMENTS}");
        let escaped_executable =
            escape_ninja_value(&ShellText::new(executable.clone()))?.to_string();
        if command_line_characters(&prefix, &command) > MAX_POWER_SHELL_COMMAND_LINE {
            let response_characters =
                command_line_characters(&executable, POWER_SHELL_RESPONSE_FILE_ARGUMENTS);
            if response_characters > MAX_POWER_SHELL_COMMAND_LINE {
                return Err(NinjaGenError::PowerShellParentCommandLineTooLong {
                    actual_characters: response_characters,
                    maximum_characters: MAX_POWER_SHELL_COMMAND_LINE,
                });
            }
            let response_command =
                format!("{escaped_executable}{POWER_SHELL_RESPONSE_FILE_ARGUMENTS}");
            return Ok(RenderedRecipeCommand::ResponseFile {
                command: NinjaValue::from_encoded(response_command),
                content: power_shell_response_file_content(&command)?,
            });
        }
        let direct_command =
            format!("{escaped_executable}{POWER_SHELL_ENCODED_ARGUMENTS}{command}");
        Ok(RenderedRecipeCommand::Direct(NinjaValue::from_encoded(
            direct_command,
        )))
    }

    /// Select the PowerShell executable expression for one binding.
    fn power_shell_executable(binding: InterpreterBinding<'_>) -> Result<String, NinjaGenError> {
        match binding {
            InterpreterBinding::Bound(executable) => {
                Ok(windows_argument(executable.as_path().as_str()))
            }
            InterpreterBinding::InspectionOnly => Ok(INSPECTION_POWER_SHELL_EXECUTABLE.to_owned()),
            InterpreterBinding::Unbound => Err(NinjaGenError::InterpreterBindingRequired {
                shell: Self::PowerShell,
            }),
        }
    }

    /// Wrap one POSIX recipe in the explicit Windows Bash compatibility runtime.
    fn bash_command(
        script: &ShellText,
        binding: InterpreterBinding<'_>,
    ) -> Result<NinjaValue, NinjaGenError> {
        let executable = match binding {
            InterpreterBinding::Bound(executable) => {
                windows_argument(executable.as_path().as_str())
            }
            InterpreterBinding::InspectionOnly => "bash.exe".to_owned(),
            InterpreterBinding::Unbound => {
                return Err(NinjaGenError::InterpreterBindingRequired { shell: Self::Bash });
            }
        };
        let command = format!("{executable} -e -c {}", windows_argument(script.as_str()));
        escape_ninja_value(&ShellText::new(command))
    }
}

#[cfg(test)]
#[path = "../shell_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../interpreter_binding_tests.rs"]
mod interpreter_binding_tests;
