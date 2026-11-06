//! Verifies default interpreter resolution and the public generation entry points.
//!
//! The library entry points resolve a Windows PowerShell binding themselves and
//! bind nothing for other shells. These tests pin that boundary: a locator
//! failure is reported rather than silently downgraded to an unbound render, a
//! dependency-only graph never consults the locator, and an unbound non-default
//! shell still refuses to render a command.
//!
//! The rendered form of an arbitrary absolute interpreter path is covered by
//! the sibling `properties` module.

use super::super::{
    InterpreterExecutable, NinjaGenError, RecipeShell, generate_with_shell,
    resolve_default_interpreter_with,
    test_support::{command_graph, dependency_only_graph, power_shell_interpreter},
};
use crate::power_shell_location::WindowsPowerShellLocationError;
use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;
use std::io;

/// One locator failure the generation boundary should report.
#[derive(Clone, Copy)]
enum LocatorFailure {
    /// The trusted executable is absent.
    Missing,
    /// The trusted path is not a regular file.
    NotRegularFile,
    /// The trusted path cannot be inspected.
    Uninspectable,
}

impl LocatorFailure {
    /// Build the failure for the path the locator inspected.
    fn error(self, expected: Utf8PathBuf) -> WindowsPowerShellLocationError {
        match self {
            Self::Missing => WindowsPowerShellLocationError::Missing {
                expected,
                source: io::Error::new(io::ErrorKind::NotFound, "no such file"),
            },
            Self::NotRegularFile => WindowsPowerShellLocationError::NotRegularFile { expected },
            Self::Uninspectable => WindowsPowerShellLocationError::Inspect {
                expected,
                source: io::Error::new(io::ErrorKind::PermissionDenied, "access denied"),
            },
        }
    }

    /// The diagnostic fragment this failure should produce.
    const fn needle(self) -> &'static str {
        match self {
            Self::Missing => "missing at",
            Self::NotRegularFile => "not a regular file",
            Self::Uninspectable => "could not inspect",
        }
    }
}

/// The trusted executable path the locator would have inspected.
fn trusted_path() -> Utf8PathBuf {
    Utf8PathBuf::from("/fixtures/Windows/System32/WindowsPowerShell/v1.0/powershell.exe")
}

/// Assert a locator failure is reported, naming `needle` and `expected`.
fn assert_locator_failure(
    expected: Option<&Utf8Path>,
    needle: &str,
    locate: impl FnOnce() -> Result<InterpreterExecutable, WindowsPowerShellLocationError>,
) {
    let error =
        resolve_default_interpreter_with(&command_graph(), RecipeShell::PowerShell, true, locate)
            .expect_err("a locator failure must not be swallowed");
    assert!(
        matches!(error, NinjaGenError::InterpreterResolutionFailed { .. }),
        "a locator failure should surface as a resolution failure: {error:?}"
    );
    let message = error.to_string();
    assert!(
        message.contains(needle),
        "the diagnostic should report {needle}: {message}"
    );
    if let Some(named) = expected {
        assert!(
            message.contains(named.as_str()),
            "the diagnostic should name {named}: {message}"
        );
    }
}

/// Assert `shell` refuses to render a command recipe without a binding.
fn assert_binding_required(shell: RecipeShell) {
    let error = generate_with_shell(&command_graph(), shell)
        .expect_err("a command recipe needs an explicit binding");
    assert!(
        matches!(
            error,
            NinjaGenError::InterpreterBindingRequired { shell: actual } if actual == shell
        ),
        "the error should name {shell:?}: {error:?}"
    );
}

/// A locator failure on a rendered recipe is reported, not silently unbound.
#[test]
fn default_resolution_reports_a_locator_failure() {
    assert_locator_failure(None, "system directory", || {
        Err(WindowsPowerShellLocationError::SystemDirectoryUnavailable)
    });
}

/// A trusted path that cannot be used names the executable it inspected.
#[rstest]
#[case::missing(LocatorFailure::Missing)]
#[case::not_regular_file(LocatorFailure::NotRegularFile)]
#[case::uninspectable(LocatorFailure::Uninspectable)]
fn default_resolution_reports_an_unusable_executable(#[case] failure: LocatorFailure) {
    let expected = trusted_path();
    let located = expected.clone();
    assert_locator_failure(Some(&expected), failure.needle(), move || {
        Err(failure.error(located))
    });
}

/// A dependency-only graph renders without consulting the locator.
#[test]
fn default_resolution_skips_dependency_only_graphs() {
    let result = resolve_default_interpreter_with(
        &dependency_only_graph(),
        RecipeShell::PowerShell,
        true,
        || panic!("a dependency-only graph must not consult the locator"),
    );
    assert!(matches!(result, Ok(None)));
}

/// A non-Windows host never consults the Windows locator.
#[test]
fn default_resolution_skips_non_windows_hosts() {
    let result =
        resolve_default_interpreter_with(&command_graph(), RecipeShell::PowerShell, false, || {
            panic!("a non-Windows host must not consult the locator")
        });
    assert!(matches!(result, Ok(None)));
}

/// Bash keeps its runner-owned resolution policy and binds nothing here.
#[test]
fn default_resolution_skips_bash() {
    let result =
        resolve_default_interpreter_with(&command_graph(), RecipeShell::Bash, true, || {
            panic!("Bash must not consult the Windows PowerShell locator")
        });
    assert!(matches!(result, Ok(None)));
}

/// A successful lookup binds the located executable for rendering.
#[test]
fn default_resolution_binds_the_located_executable() {
    let expected = power_shell_interpreter();
    let result =
        resolve_default_interpreter_with(&command_graph(), RecipeShell::PowerShell, true, || {
            Ok(power_shell_interpreter())
        });
    let resolved = result
        .expect("a successful lookup should resolve")
        .expect("a rendered recipe should carry a binding");
    assert_eq!(resolved.as_path(), expected.as_path());
}

/// A non-default shell entry point refuses to render without a binding.
#[test]
fn generate_with_shell_requires_a_binding_for_bash() {
    assert_binding_required(RecipeShell::Bash);
}

/// Off Windows, PowerShell rendering also needs an explicit binding.
#[cfg(not(windows))]
#[test]
fn generate_with_shell_requires_a_binding_for_power_shell_off_windows() {
    assert_binding_required(RecipeShell::PowerShell);
}

/// A dependency-only graph renders through the default entry point unbound.
#[rstest]
#[case::bash(RecipeShell::Bash)]
#[case::power_shell(RecipeShell::PowerShell)]
fn generate_with_shell_renders_dependency_only_graphs(#[case] shell: RecipeShell) {
    let ninja = generate_with_shell(&dependency_only_graph(), shell)
        .expect("a dependency-only graph needs no interpreter binding");
    assert!(
        !ninja.contains("powershell.exe"),
        "a dependency-only graph must not name an interpreter: {ninja}"
    );
}
