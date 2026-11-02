//! Bind the trusted Windows PowerShell interpreter used by generated recipes.
//!
//! The locator in [`crate::recipe_shell`] owns the trusted lookup policy. This
//! runner adapter turns its typed failures into actionable diagnostics and
//! deliberately offers no `PATH`, application-directory, current-directory,
//! `SystemRoot`, `WINDIR`, or `PSHOME` fallback.

use anyhow::Result;

use crate::recipe_shell::{
    InterpreterExecutable, KnownFolderSystemDirectory, SystemDirectory,
    WindowsPowerShellLocationError, locate_windows_power_shell,
};

use super::WINDOWS_SHELL_ENV;

/// Fixed engine path named when the host has no usable system directory.
const POWER_SHELL_FALLBACK_DESCRIPTION: &str =
    "WindowsPowerShell\\v1.0\\powershell.exe beneath the Windows system directory";

/// Resolve the trusted Windows PowerShell executable for generated recipes.
///
/// # Errors
///
/// Returns an error naming the expected executable path when the Known Folder
/// lookup, filesystem inspection, or executable binding fails.
pub(in crate::runner) fn resolve_power_shell_runtime() -> Result<InterpreterExecutable> {
    resolve_power_shell_runtime_with(&KnownFolderSystemDirectory)
}

/// Resolve Windows PowerShell through one injected system-directory provider.
///
/// # Errors
///
/// Returns an error when the provider supplies no usable system directory or
/// the fixed engine path is missing, is not a regular non-reparse file, or
/// cannot be bound as an [`InterpreterExecutable`].
fn resolve_power_shell_runtime_with(
    provider: &impl SystemDirectory,
) -> Result<InterpreterExecutable> {
    locate_windows_power_shell(provider).map_err(power_shell_binding_error)
}

/// Render one locator failure as a runner diagnostic.
fn power_shell_binding_error(error: WindowsPowerShellLocationError) -> anyhow::Error {
    let description = error.expected_path().map_or_else(
        || {
            format!(
                "could not bind the Windows PowerShell interpreter at {POWER_SHELL_FALLBACK_DESCRIPTION}"
            )
        },
        |expected| format!("could not bind the Windows PowerShell interpreter at `{expected}`"),
    );
    let message = format!(
        "{description}: {error}; Netsuke binds Windows PowerShell only through the Windows \
         system directory and never searches PATH or the current directory, so a workspace file \
         cannot shadow the interpreter; set {WINDOWS_SHELL_ENV}=bash to use a Bash runtime \
         instead"
    );
    anyhow::Error::new(error).context(message)
}

#[cfg(test)]
#[path = "locator_runtime_tests.rs"]
mod tests;
