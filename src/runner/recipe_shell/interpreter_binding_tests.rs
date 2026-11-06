//! Verify interpreter dispatch selects only the lookup required by the shell.

use super::resolve_interpreter_binding_with;
use crate::recipe_shell::{InterpreterExecutable, RecipeShell};

/// Build one absolute fixture executable for interpreter dispatch tests.
fn fixture_executable() -> InterpreterExecutable {
    let path = if cfg!(windows) {
        "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"
    } else {
        "/bin/sh"
    };
    InterpreterExecutable::from_absolute_path(path.into())
        .expect("fixture executable path should be absolute")
}

/// Windows PowerShell binding must not consult the Bash lookup.
#[test]
fn windows_power_shell_binding_skips_the_bash_lookup() -> anyhow::Result<()> {
    let mut power_shell_calls = 0_usize;
    let mut bash_calls = 0_usize;

    let binding = resolve_interpreter_binding_with(
        RecipeShell::PowerShell,
        true,
        || {
            power_shell_calls += 1;
            Ok(fixture_executable())
        },
        || {
            bash_calls += 1;
            Ok(None)
        },
    )?;

    anyhow::ensure!(binding.is_some(), "PowerShell should bind an executable");
    anyhow::ensure!(
        power_shell_calls == 1,
        "PowerShell lookup should run exactly once"
    );
    anyhow::ensure!(
        bash_calls == 0,
        "PowerShell binding must not consult the Bash lookup"
    );
    Ok(())
}

/// Windows Bash binding must not consult the PowerShell locator.
#[test]
fn windows_bash_binding_skips_the_power_shell_lookup() -> anyhow::Result<()> {
    let mut power_shell_calls = 0_usize;
    let mut bash_calls = 0_usize;

    let binding = resolve_interpreter_binding_with(
        RecipeShell::Bash,
        true,
        || {
            power_shell_calls += 1;
            Ok(fixture_executable())
        },
        || {
            bash_calls += 1;
            Ok(Some(fixture_executable()))
        },
    )?;

    anyhow::ensure!(binding.is_some(), "Bash should bind an executable");
    anyhow::ensure!(bash_calls == 1, "Bash lookup should run exactly once");
    anyhow::ensure!(
        power_shell_calls == 0,
        "Bash binding must not run the PowerShell locator"
    );
    Ok(())
}

/// Posix recipes and non-Windows hosts bind no interpreter.
#[rstest::rstest]
#[case::posix_on_windows(RecipeShell::Posix, true)]
#[case::power_shell_off_windows(RecipeShell::PowerShell, false)]
#[case::bash_off_windows(RecipeShell::Bash, false)]
fn interpreter_dispatch_skips_every_lookup(
    #[case] shell: RecipeShell,
    #[case] is_windows: bool,
) -> anyhow::Result<()> {
    let lookup_calls = std::cell::Cell::new(0_usize);

    let binding = resolve_interpreter_binding_with(
        shell,
        is_windows,
        || {
            lookup_calls.set(lookup_calls.get() + 1);
            Ok(fixture_executable())
        },
        || {
            lookup_calls.set(lookup_calls.get() + 1);
            Ok(None)
        },
    )?;

    anyhow::ensure!(binding.is_none(), "no interpreter should be bound");
    anyhow::ensure!(
        lookup_calls.get() == 0,
        "dispatch must not run an interpreter lookup for {shell:?} on Windows={is_windows}"
    );
    Ok(())
}
