//! Windows-specific injected `PATH` and `PATHEXT` capture tests.

use super::*;

#[test]
fn capture_uses_injected_and_normalized_pathext() {
    let env = mock_env_for_capture(
        Some(OsString::from(r"C:\configured\bin")),
        Some(OsString::from(".EXE;exe; CMD ;.cmd")),
        Err(std::env::VarError::NotPresent),
    );

    let snapshot = EnvSnapshot::capture_with_env(Some(Utf8Path::new("C:/workspace")), None, &env)
        .expect("injected PATH and PATHEXT should produce an environment snapshot");

    assert_eq!(snapshot.pathext(), [".exe", ".cmd"]);
}

#[test]
fn capture_uses_default_pathext_when_injected_values_are_unset() {
    let env = mock_env_for_capture(None, None, Err(std::env::VarError::NotPresent));

    let snapshot = EnvSnapshot::capture_with_env(Some(Utf8Path::new("C:/workspace")), None, &env)
        .expect("unset injected PATH and PATHEXT should produce an environment snapshot");

    assert_eq!(snapshot.raw_path, None);
    assert_eq!(snapshot.raw_pathext, None);
    assert_eq!(snapshot.pathext(), DEFAULT_PATHEXT);
}
