//! Resolves and validates the Windows legacy-recipe interpreter selection.

mod bash_runtime;
pub(in crate::runner) mod telemetry;
pub(super) use bash_runtime::resolve_bash_runtime;

use anyhow::{Result, bail};
use mockable::Env;
use std::ffi::OsString;

use crate::recipe_shell::RecipeShell;

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
