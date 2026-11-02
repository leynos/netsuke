//! Verify trusted Windows PowerShell location without host environment access.

use std::path::{Path, PathBuf};

#[cfg(not(windows))]
use super::KnownFolderSystemDirectory;
use super::{SystemDirectory, WindowsPowerShellLocationError, locate_windows_power_shell};

/// Supply one fixed system directory to the locator.
struct FixedSystemDirectory(Option<PathBuf>);

impl SystemDirectory for FixedSystemDirectory {
    fn system_directory(&self) -> Option<PathBuf> {
        self.0.clone()
    }
}

/// Return the engine directory beneath one fixture system directory.
fn engine_directory(system_directory: &Path) -> PathBuf {
    system_directory.join("WindowsPowerShell").join("v1.0")
}

/// Create a regular `powershell.exe` beneath `system_directory`.
fn write_power_shell(system_directory: &Path) -> anyhow::Result<PathBuf> {
    let directory = engine_directory(system_directory);
    test_support::fs::create_dir_all(&directory)?;
    let executable = directory.join("powershell.exe");
    test_support::fs::write(&executable, b"fixture executable")?;
    Ok(executable)
}

/// Bind one fixture system directory to the locator.
fn with_system_directory(system_directory: Option<&Path>) -> FixedSystemDirectory {
    FixedSystemDirectory(system_directory.map(Path::to_path_buf))
}

/// Require the locator to reject one fixture system directory.
fn locate_error(system_directory: Option<&Path>) -> WindowsPowerShellLocationError {
    locate_windows_power_shell(&with_system_directory(system_directory))
        .expect_err("the fixture system directory should not bind")
}

#[test]
fn locates_the_fixed_engine_path_beneath_the_supplied_system_directory() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    write_power_shell(workspace.path())?;

    let located = locate_windows_power_shell(&with_system_directory(Some(workspace.path())))
        .expect("a regular engine file should be bound");

    anyhow::ensure!(
        located
            .as_path()
            .ends_with("WindowsPowerShell/v1.0/powershell.exe"),
        "locator should bind the fixed engine path, got {}",
        located.as_path()
    );
    anyhow::ensure!(
        located.as_path().is_absolute(),
        "bound executable path should remain absolute"
    );
    Ok(())
}

#[test]
fn missing_engine_file_names_the_expected_path() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    test_support::fs::create_dir_all(engine_directory(workspace.path()))?;

    let error = locate_error(Some(workspace.path()));

    let expected = error.expected_path().expect("failure should name a path");
    anyhow::ensure!(
        expected.ends_with("WindowsPowerShell/v1.0/powershell.exe"),
        "failure should name the fixed engine path"
    );
    anyhow::ensure!(
        error.to_string().contains(expected.as_str()),
        "diagnostic should include the expected path: {error}"
    );
    Ok(())
}

#[test]
fn rejects_a_directory_at_the_engine_path() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    test_support::fs::create_dir_all(engine_directory(workspace.path()).join("powershell.exe"))?;

    let error = locate_error(Some(workspace.path()));

    anyhow::ensure!(
        matches!(error, WindowsPowerShellLocationError::NotRegularFile { .. }),
        "expected a regular-file rejection, got {error:?}"
    );
    Ok(())
}

/// A symlink at the engine path is refused rather than followed.
///
/// The final component is inspected without following it, so a link to an
/// otherwise acceptable file cannot substitute for the trusted executable.
#[cfg(unix)]
#[test]
fn rejects_a_symlink_at_the_engine_path() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let target = workspace.path().join("real-powershell");
    test_support::fs::write(&target, b"fixture executable")?;
    let engine = engine_directory(workspace.path());
    test_support::fs::create_dir_all(&engine)?;
    test_support::fs::symlink(&target, engine.join("powershell.exe"))?;

    let error = locate_error(Some(workspace.path()));

    anyhow::ensure!(
        matches!(error, WindowsPowerShellLocationError::NotRegularFile { .. }),
        "expected a symlink rejection, got {error:?}"
    );
    Ok(())
}

#[test]
fn rejects_a_relative_system_directory() {
    let error = locate_error(Some(Path::new("relative")));

    assert!(
        matches!(
            error,
            WindowsPowerShellLocationError::RelativeSystemDirectory { .. }
        ),
        "expected a relative-directory rejection, got {error:?}"
    );
}

#[test]
fn reports_an_unavailable_system_directory() {
    let error = locate_error(None);

    assert!(
        matches!(
            error,
            WindowsPowerShellLocationError::SystemDirectoryUnavailable
        ),
        "expected an unavailable-directory failure, got {error:?}"
    );
}

#[cfg(unix)]
#[test]
fn rejects_a_non_utf8_system_directory() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt as _;

    let path = PathBuf::from(OsString::from_vec(b"/tmp/netsuke-non-utf8-\xff".to_vec()));

    let error = locate_error(Some(&path));

    assert!(
        matches!(
            error,
            WindowsPowerShellLocationError::NonUtf8SystemDirectory { .. }
        ),
        "expected a non-UTF-8 directory rejection, got {error:?}"
    );
}

#[test]
fn locator_uses_only_the_supplied_system_directory() -> anyhow::Result<()> {
    let trusted = tempfile::tempdir()?;
    let decoy = tempfile::tempdir()?;
    write_power_shell(trusted.path())?;
    write_power_shell(decoy.path())?;

    let located = locate_windows_power_shell(&with_system_directory(Some(trusted.path())))
        .expect("trusted engine file should be bound");

    anyhow::ensure!(
        located.as_path().starts_with(trusted.path()),
        "locator should bind the supplied directory, got {}",
        located.as_path()
    );
    anyhow::ensure!(
        !located.as_path().starts_with(decoy.path()),
        "locator must ignore any other candidate directory"
    );
    Ok(())
}

#[cfg(not(windows))]
#[test]
fn production_provider_has_no_system_directory_off_windows() {
    assert_eq!(
        KnownFolderSystemDirectory.system_directory(),
        None,
        "non-Windows hosts must not attempt a Windows system-directory lookup"
    );
}
