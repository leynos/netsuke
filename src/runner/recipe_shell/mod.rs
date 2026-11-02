//! Resolves and validates the Windows legacy-recipe interpreter selection.

mod bash_runtime;
mod power_shell_runtime;
pub(in crate::runner) mod telemetry;
pub(super) use bash_runtime::resolve_bash_runtime;

use anyhow::{Result, bail};
use mockable::Env;
use std::ffi::OsString;

use crate::recipe_shell::{InterpreterExecutable, RecipeShell};

use power_shell_runtime::resolve_power_shell_runtime;

/// Names the optional Windows legacy-recipe interpreter override.
pub(super) const WINDOWS_SHELL_ENV: &str = "NETSUKE_WINDOWS_SHELL";

/// Resolve the current host's legacy-recipe interpreter selection.
pub(super) fn resolve_recipe_shell() -> Result<RecipeShell> {
    telemetry::instrument_recipe_shell_resolution(|| {
        resolve_recipe_shell_with(&mockable::DefaultEnv)
    })
}

/// Resolve the current host's legacy-recipe interpreter with an injected environment.
pub(super) fn resolve_recipe_shell_with(env: &impl Env) -> Result<RecipeShell> {
    if !cfg!(windows) {
        return Ok(RecipeShell::Posix);
    }
    resolve_windows_recipe_shell(env.os_string(WINDOWS_SHELL_ENV))
}

/// Resolve the executable bound to the selected legacy-recipe interpreter.
///
/// Windows PowerShell is bound through the trusted system-directory locator so
/// that generated recipes never resolve an unqualified `powershell.exe`. A
/// Windows Bash selection keeps its existing explicit `PATH` policy. Posix
/// recipes and non-Windows hosts bind nothing because Ninja launches their
/// interpreter itself.
///
/// # Errors
///
/// Returns an error when the selected interpreter cannot be located or
/// validated.
pub(in crate::runner) fn resolve_interpreter_binding(
    shell: RecipeShell,
) -> Result<Option<InterpreterExecutable>> {
    resolve_interpreter_binding_with(shell, cfg!(windows), resolve_power_shell_runtime, || {
        resolve_bash_runtime(shell)
    })
}

/// Dispatch interpreter binding with the platform and lookups injected.
///
/// Each branch consults at most one lookup so a PowerShell build can never
/// probe Bash, and a Bash build can never inspect the Windows system
/// directory.
fn resolve_interpreter_binding_with(
    shell: RecipeShell,
    is_windows: bool,
    resolve_power_shell: impl FnOnce() -> Result<InterpreterExecutable>,
    resolve_bash: impl FnOnce() -> Result<Option<InterpreterExecutable>>,
) -> Result<Option<InterpreterExecutable>> {
    if !is_windows {
        return Ok(None);
    }
    match shell {
        RecipeShell::PowerShell => resolve_power_shell().map(Some),
        RecipeShell::Bash => resolve_bash(),
        RecipeShell::Posix => Ok(None),
    }
}

/// Resolve the configured Windows recipe shell from one raw environment value.
fn resolve_windows_recipe_shell(raw_value: Option<OsString>) -> Result<RecipeShell> {
    let Some(shell_value) = raw_value else {
        return Ok(RecipeShell::PowerShell);
    };
    let shell_name = shell_value.into_string().map_err(|invalid_value| {
        anyhow::anyhow!(
            "{WINDOWS_SHELL_ENV} must be valid Unicode, received {}",
            invalid_value.to_string_lossy()
        )
    })?;
    match shell_name.trim().to_ascii_lowercase().as_str() {
        "" | "powershell" => Ok(RecipeShell::PowerShell),
        "bash" => Ok(RecipeShell::Bash),
        _ => bail!(
            "{WINDOWS_SHELL_ENV} must be `powershell` or `bash`; \
             omit it to use the Windows PowerShell default"
        ),
    }
}

#[cfg(test)]
mod tests {
    //! Verifies Windows legacy-recipe interpreter selection.

    use super::{WINDOWS_SHELL_ENV, resolve_recipe_shell_with, resolve_windows_recipe_shell};
    use crate::recipe_shell::RecipeShell;
    use mockable::MockEnv;
    use std::ffi::OsString;

    /// Build an injected environment for recipe-shell resolution tests.
    fn recipe_shell_env(value: Option<&str>) -> MockEnv {
        let mut env = MockEnv::new();
        env.expect_os_string()
            .times(usize::from(cfg!(windows)))
            .withf(|key| key == WINDOWS_SHELL_ENV)
            .return_const(value.map(OsString::from));
        env
    }

    #[test]
    fn defaults_to_the_host_recipe_shell() {
        let shell = resolve_recipe_shell_with(&recipe_shell_env(None))
            .expect("default shell resolution should succeed");
        assert_eq!(shell, RecipeShell::host_default());
    }

    #[test]
    fn defaults_windows_to_power_shell() {
        let shell = resolve_windows_recipe_shell(None)
            .expect("Windows default shell resolution should succeed");
        assert_eq!(shell, RecipeShell::PowerShell);
    }

    #[test]
    fn accepts_the_explicit_bash_compatibility_selection() {
        let shell = resolve_windows_recipe_shell(Some(OsString::from("bash")))
            .expect("bash selection should succeed");
        assert_eq!(shell, RecipeShell::Bash);
    }

    /// Verify that PowerShell override spellings select the native Windows route.
    #[rstest::rstest]
    #[case::explicit("powershell")]
    #[case::empty("")]
    #[case::whitespace("  \t  ")]
    #[case::case_and_whitespace("  PoWeRsHeLl\t")]
    fn accepts_power_shell_override_variants(#[case] value: &str) {
        let shell = resolve_windows_recipe_shell(Some(OsString::from(value)))
            .expect("PowerShell override should resolve");
        assert_eq!(shell, RecipeShell::PowerShell);
    }

    #[test]
    fn rejects_an_unknown_windows_recipe_shell() {
        let error = resolve_windows_recipe_shell(Some(OsString::from("cmd")))
            .expect_err("unknown shell selection should fail");
        assert!(error.to_string().contains("powershell` or `bash"));
    }
}

#[cfg(test)]
#[path = "interpreter_binding_tests.rs"]
mod interpreter_binding_tests;
