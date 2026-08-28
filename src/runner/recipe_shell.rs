//! Resolves and validates the Windows legacy-recipe interpreter selection.

use anyhow::{Context, Result, bail};
use mockable::Env;
use std::ffi::OsString;

/// Record one outcome from the Bash availability probe.
#[derive(Debug, Eq, PartialEq)]
enum BashProbeStatus {
    /// Confirm that the runtime accepted `--version`.
    Available,
    /// Preserve the runtime's failed status for the actionable diagnostic.
    Failed(String),
}

use crate::ninja_gen::RecipeShell;

/// Names the optional Windows legacy-recipe interpreter override.
pub(super) const WINDOWS_SHELL_ENV: &str = "NETSUKE_WINDOWS_SHELL";

/// Resolve the current host's legacy-recipe interpreter selection.
pub(super) fn resolve_recipe_shell() -> Result<RecipeShell> {
    resolve_recipe_shell_with(&mockable::DefaultEnv)
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

/// Confirm that an explicitly selected external recipe runtime can start.
pub(super) fn validate_recipe_shell(shell: RecipeShell) -> Result<()> {
    if !cfg!(windows) || shell != RecipeShell::Bash {
        return Ok(());
    }
    validate_bash_runtime_with(|| {
        std::process::Command::new("bash.exe")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| {
                if status.success() {
                    BashProbeStatus::Available
                } else {
                    BashProbeStatus::Failed(status.to_string())
                }
            })
    })
}

/// Validate one injected Bash availability probe.
fn validate_bash_runtime_with(
    probe: impl FnOnce() -> std::io::Result<BashProbeStatus>,
) -> Result<()> {
    let probe_status = probe().context(
        "Windows legacy recipes selected `bash`, but `bash.exe` was not found on PATH; \
             install Git for Windows or MSYS2, add its Bash directory to PATH, or unset \
             NETSUKE_WINDOWS_SHELL to use PowerShell",
    )?;
    if let BashProbeStatus::Failed(status) = probe_status {
        bail!(
            "Windows legacy recipes selected `bash`, but `bash.exe --version` exited with {status}; \
             repair the Bash runtime or unset NETSUKE_WINDOWS_SHELL to use PowerShell"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Verifies Windows legacy-recipe interpreter selection.

    use super::{
        BashProbeStatus, WINDOWS_SHELL_ENV, resolve_recipe_shell_with,
        resolve_windows_recipe_shell, validate_bash_runtime_with,
    };
    use crate::ninja_gen::RecipeShell;
    use mockable::MockEnv;
    use std::ffi::OsString;

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

    #[test]
    fn rejects_an_unknown_windows_recipe_shell() {
        let error = resolve_windows_recipe_shell(Some(OsString::from("cmd")))
            .expect_err("unknown shell selection should fail");
        assert!(error.to_string().contains("powershell` or `bash"));
    }

    #[test]
    fn bash_runtime_probe_accepts_a_successful_runtime() {
        let result = validate_bash_runtime_with(|| Ok(BashProbeStatus::Available));
        assert!(result.is_ok());
    }

    #[test]
    fn bash_runtime_probe_reports_a_missing_runtime() {
        let error = validate_bash_runtime_with(|| {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "missing bash",
            ))
        })
        .expect_err("missing Bash should be actionable");
        assert!(
            error
                .to_string()
                .contains("bash.exe` was not found on PATH")
        );
    }

    #[test]
    fn bash_runtime_probe_reports_an_unsuccessful_runtime() {
        let error =
            validate_bash_runtime_with(|| Ok(BashProbeStatus::Failed("exit status: 7".into())))
                .expect_err("failed Bash probe should be actionable");
        assert!(
            error
                .to_string()
                .contains("bash.exe --version` exited with exit status: 7")
        );
    }
}
