//! Windows-specific injected `PATH` and `PATHEXT` capture tests.

use super::*;
use mockable::MockEnv;

#[test]
fn capture_uses_injected_and_normalized_pathext() {
    let mut env = MockEnv::new();
    env.expect_os_string()
        .withf(|key| key == "PATH")
        .once()
        .return_once(|_| Some(OsString::from(r"C:\configured\bin")));
    env.expect_os_string()
        .withf(|key| key == "PATHEXT")
        .once()
        .return_once(|_| Some(OsString::from(".EXE;exe; CMD ;.cmd")));
    // Capture also reads the workspace switch through the same provider;
    // pinning the key keeps that read observable rather than wildcarded.
    env.expect_raw()
        .withf(|key| key == WORKSPACE_FALLBACK_ENV)
        .once()
        .return_const(Err(std::env::VarError::NotPresent));

    let snapshot = EnvSnapshot::capture_with_env(Some(Utf8Path::new("C:/workspace")), None, &env)
        .expect("injected PATH and PATHEXT should produce an environment snapshot");

    assert_eq!(snapshot.pathext(), [".exe", ".cmd"]);
}
