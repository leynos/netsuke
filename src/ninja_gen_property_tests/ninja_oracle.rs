//! Real-Ninja command-oracle support for Ninja generation properties.

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use proptest::prelude::*;
use std::{process::Command, sync::OnceLock};
use test_support::ninja::ninja_integration_workspace;

/// Generate ordinary and explicitly braced scalar shell commands.
pub(super) fn scalar_command_strategy() -> impl Strategy<Value = (String, String)> {
    prop::collection::vec(
        prop_oneof![
            Just("plain"),
            Just("two words"),
            Just("$value"),
            Just("${value:-fallback}"),
            Just("$(command)"),
            Just("'quoted'"),
            Just("\"double\""),
            Just("@!?*-"),
        ],
        1..8,
    )
    .prop_map(|parts| {
        let command = format!("echo {}", parts.join(" "));
        let braced_command = format!("{command} ${{value:-fallback}}");
        (command, braced_command)
    })
}

/// Report whether the local test environment can use the real Ninja oracle.
pub(super) fn ninja_is_available() -> bool {
    static NINJA_AVAILABLE: OnceLock<bool> = OnceLock::new();
    *NINJA_AVAILABLE.get_or_init(|| ninja_integration_workspace().is_ok())
}

/// Ask Ninja to parse a generated file and return its expanded command text.
pub(super) fn ninja_commands(ninja_file: &str) -> Result<String, TestCaseError> {
    let workspace = ninja_integration_workspace()
        .map_err(|error| TestCaseError::fail(format!("create Ninja workspace: {error}")))?;
    let path = Utf8PathBuf::from_path_buf(workspace.path().to_path_buf()).map_err(|non_utf8| {
        TestCaseError::fail(format!("non-UTF-8 temporary path: {non_utf8:?}"))
    })?;
    let directory = Dir::open_ambient_dir(&path, ambient_authority())
        .map_err(|error| TestCaseError::fail(format!("open Ninja workspace {path}: {error}")))?;
    directory
        .write("build.ninja", ninja_file)
        .map_err(|error| TestCaseError::fail(format!("write generated Ninja file: {error}")))?;

    let output = Command::new("ninja")
        .args(["-f", "build.ninja", "-t", "commands", "out"])
        .current_dir(path.as_std_path())
        .output()
        .map_err(|error| TestCaseError::fail(format!("run Ninja command oracle: {error}")))?;
    if !output.status.success() {
        return Err(TestCaseError::fail(format!(
            "Ninja rejected generated file: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    String::from_utf8(output.stdout).map_err(|error| {
        TestCaseError::fail(format!("Ninja command output was not UTF-8: {error}"))
    })
}
