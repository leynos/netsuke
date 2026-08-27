//! Renders legacy recipes for the interpreter selected by the host contract.

use base64::{Engine as _, engine::general_purpose::STANDARD};

use super::NinjaGenError;
use super::ninja_gen_escape::{NinjaValue, ShellText, escape_ninja_value};

/// Selects the interpreter that receives completed legacy recipe text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecipeShell {
    /// Uses the host POSIX shell through Ninja's ordinary Unix execution path.
    Posix,
    /// Uses Windows PowerShell with an encoded script argument.
    PowerShell,
    /// Uses an explicitly selected Bash compatibility runtime on Windows.
    Bash,
}

impl RecipeShell {
    /// Return the interpreter Netsuke selects when no Windows override exists.
    pub(crate) const fn host_default() -> Self {
        if cfg!(windows) {
            Self::PowerShell
        } else {
            Self::Posix
        }
    }

    /// Render completed recipe text as a safe Ninja command binding value.
    pub(super) fn command_value(self, script: &ShellText) -> Result<NinjaValue, NinjaGenError> {
        match self {
            Self::Posix => escape_ninja_value(script),
            Self::PowerShell => {
                let power_shell_script = Self::power_shell_script(script);
                Self::power_shell_command(&power_shell_script)
            }
            Self::Bash => Self::bash_command(script),
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
    fn power_shell_command(script: &ShellText) -> Result<NinjaValue, NinjaGenError> {
        if script.as_str().contains('\0') {
            return Err(NinjaGenError::UnsafeNinjaValue);
        }
        let utf16le = script
            .as_str()
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let command = STANDARD.encode(utf16le);
        Ok(NinjaValue::from_encoded(format!(
            "powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand {command}"
        )))
    }

    /// Wrap one POSIX recipe in the explicit Windows Bash compatibility runtime.
    fn bash_command(script: &ShellText) -> Result<NinjaValue, NinjaGenError> {
        let command = format!("bash.exe -e -c {}", windows_argument(script.as_str()));
        escape_ninja_value(&ShellText::new(command))
    }
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

    use super::{RecipeShell, windows_argument};
    use crate::ninja_gen::ninja_gen_escape::ShellText;

    #[test]
    fn power_shell_command_hides_recipe_dollars_from_ninja() {
        let rendered = RecipeShell::PowerShell
            .command_value(&ShellText::new("$env:NETSUKE_SMOKE".into()))
            .expect("PowerShell encoding should succeed")
            .to_string();
        assert!(rendered.starts_with("powershell.exe "));
        assert!(!rendered.contains("NETSUKE_SMOKE"));
    }

    #[test]
    fn windows_argument_preserves_quotes_and_trailing_backslashes() {
        assert_eq!(
            windows_argument("a \\\"b\\\"\\\\"),
            "\"a \\\\\\\"b\\\\\\\"\\\\\\\\\""
        );
    }
}
