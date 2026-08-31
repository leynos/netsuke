//! Resolves and validates the Windows legacy-recipe interpreter selection.

use anyhow::{Result, bail};
use mockable::Env;
use std::{
    ffi::OsString,
    process::{Command, Stdio},
};

/// Record one outcome from the Bash availability probe.
#[derive(Debug, Eq, PartialEq)]
enum BashProbeStatus {
    /// Confirm that the runtime accepted `--version`.
    Available,
    /// Preserve the runtime's failed status for the actionable diagnostic.
    Failed(String),
}

use crate::recipe_shell::RecipeShell;

/// Names the optional Windows legacy-recipe interpreter override.
pub(super) const WINDOWS_SHELL_ENV: &str = "NETSUKE_WINDOWS_SHELL";

/// Resolve the current host's legacy-recipe interpreter selection.
pub(super) fn resolve_recipe_shell() -> Result<RecipeShell> {
    super::recipe_shell_telemetry::instrument_recipe_shell_resolution(|| {
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

/// Confirm that an explicitly selected external recipe runtime can start.
pub(super) fn validate_recipe_shell(shell: RecipeShell) -> Result<()> {
    validate_recipe_shell_with(cfg!(windows), shell, probe_bash_runtime)
}

/// Validate a selected shell with an injected Bash probe at the host boundary.
fn validate_recipe_shell_with(
    is_windows: bool,
    shell: RecipeShell,
    probe: impl FnOnce() -> std::io::Result<BashProbeStatus>,
) -> Result<()> {
    if !is_windows || shell != RecipeShell::Bash {
        return Ok(());
    }
    let mut probe_outcome = super::recipe_shell_telemetry::BashProbeOutcome::LaunchFailed;
    let validation = validate_bash_runtime_with(|| {
        let probe_result = probe();
        probe_outcome = bash_probe_outcome(&probe_result);
        probe_result
    });
    super::recipe_shell_telemetry::instrument_bash_preflight(probe_outcome, || validation)
}

/// Probe the production Bash compatibility runtime without leaking child output.
fn probe_bash_runtime() -> std::io::Result<BashProbeStatus> {
    Command::new("bash.exe")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| {
            if status.success() {
                BashProbeStatus::Available
            } else {
                BashProbeStatus::Failed(status.to_string())
            }
        })
}

/// Validate one injected Bash availability probe.
fn validate_bash_runtime_with(
    probe: impl FnOnce() -> std::io::Result<BashProbeStatus>,
) -> Result<()> {
    validate_bash_probe_result(probe())
}

/// Validate one completed Bash availability probe result.
fn validate_bash_probe_result(probe_result: std::io::Result<BashProbeStatus>) -> Result<()> {
    let probe_status = probe_result.map_err(|error| {
        let message = if error.kind() == std::io::ErrorKind::NotFound {
            "Windows legacy recipes selected `bash`, but `bash.exe` was not found on PATH; \
             install Git for Windows or MSYS2, add its Bash directory to PATH, or unset \
             NETSUKE_WINDOWS_SHELL to use PowerShell"
        } else {
            "Windows legacy recipes selected `bash`, but Netsuke could not start `bash.exe`; \
             repair the Bash runtime or unset NETSUKE_WINDOWS_SHELL to use PowerShell"
        };
        anyhow::Error::new(error).context(message)
    })?;
    if let BashProbeStatus::Failed(status) = probe_status {
        bail!(
            "Windows legacy recipes selected `bash`, but `bash.exe --version` exited with {status}; \
             repair the Bash runtime or unset NETSUKE_WINDOWS_SHELL to use PowerShell"
        );
    }
    Ok(())
}

/// Classify a Bash probe result without recording process or environment detail.
fn bash_probe_outcome(
    probe_result: &std::io::Result<BashProbeStatus>,
) -> super::recipe_shell_telemetry::BashProbeOutcome {
    match probe_result {
        Ok(BashProbeStatus::Available) => super::recipe_shell_telemetry::BashProbeOutcome::Success,
        Ok(BashProbeStatus::Failed(_)) => {
            super::recipe_shell_telemetry::BashProbeOutcome::NonZeroExit
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            super::recipe_shell_telemetry::BashProbeOutcome::NotFound
        }
        Err(_) => super::recipe_shell_telemetry::BashProbeOutcome::LaunchFailed,
    }
}

#[cfg(test)]
mod tests {
    //! Verifies Windows legacy-recipe interpreter selection.

    use super::{
        BashProbeStatus, WINDOWS_SHELL_ENV, resolve_recipe_shell_with,
        resolve_windows_recipe_shell, validate_bash_runtime_with, validate_recipe_shell_with,
    };
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

    /// Verify that a successful injected Bash probe permits the compatibility route.
    #[test]
    fn bash_runtime_probe_accepts_a_successful_runtime() {
        let result = validate_bash_runtime_with(|| Ok(BashProbeStatus::Available));
        assert!(result.is_ok());
    }

    /// Verify that a missing injected Bash runtime produces actionable guidance.
    #[test]
    fn bash_runtime_probe_reports_a_missing_runtime() {
        assert_bash_runtime_launch_error(std::io::ErrorKind::NotFound);
    }

    /// Verify that a non-NotFound process launch failure remains actionable.
    #[test]
    fn bash_runtime_probe_reports_a_launch_failure() {
        assert_bash_runtime_launch_error(std::io::ErrorKind::PermissionDenied);
    }

    /// Assert that a Bash process-launch error produces the matching diagnostic.
    fn assert_bash_runtime_launch_error(error_kind: std::io::ErrorKind) {
        let error = validate_bash_runtime_with(|| {
            Err(std::io::Error::new(error_kind, "cannot launch bash"))
        })
        .expect_err("launch failure should be actionable");
        let expected = if error_kind == std::io::ErrorKind::NotFound {
            "bash.exe` was not found on PATH"
        } else {
            "could not start `bash.exe`"
        };
        assert!(error.to_string().contains(expected));
    }

    /// Verify that a failing injected Bash probe preserves its exit diagnostic.
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

    /// Avoid probing Bash when a selected shell does not require it.
    #[test]
    fn non_bash_shells_skip_bash_preflight() {
        for shell in [RecipeShell::Posix, RecipeShell::PowerShell] {
            let result = validate_recipe_shell_with(true, shell, || {
                panic!("{shell:?} must not invoke the Bash probe")
            });
            assert!(result.is_ok());
        }
    }
}
