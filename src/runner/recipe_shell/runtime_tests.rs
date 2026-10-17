//! Verify injected Bash lookup, probing, and diagnostic boundaries.

use super::{
    BashProbeStatus, BashRuntimeContext, MAX_PROBE_OUTPUT_BYTES, describe_probe_output,
    read_bounded_and_drain, resolve_bash_from_path, resolve_bash_runtime_with, sanitized_output,
    validate_bash_probe,
};
use crate::recipe_shell::RecipeShell;
use crate::{
    ast::{CommandIsolation, Recipe, StringOrList},
    ir::{Action, BuildGraph},
};
use camino::{Utf8Path, Utf8PathBuf};
use mockable::MockEnv;
use std::{ffi::OsString, io::Cursor, path::Path};

/// Create a regular `bash.exe` candidate in one fixture directory.
fn create_bash_candidate(directory: &Path) -> anyhow::Result<std::path::PathBuf> {
    test_support::fs::create_dir_all(directory)?;
    let executable = directory.join("bash.exe");
    test_support::fs::write(&executable, b"fixture executable")?;
    Ok(executable)
}

/// Build a mock environment exposing one injected `PATH` value.
fn path_env(path: Option<OsString>) -> MockEnv {
    let mut env = MockEnv::new();
    env.expect_os_string()
        .withf(|key| key == "PATH")
        .once()
        .return_const(path);
    env
}

/// Join fixture directories with the current platform's path-list syntax.
fn joined_path(directories: &[&Path]) -> anyhow::Result<OsString> {
    Ok(std::env::join_paths(directories)?)
}

/// Resolve the first regular Bash executable in the injected `PATH` order.
#[test]
fn bash_resolution_uses_the_first_path_match() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let first_directory = workspace.path().join("first");
    let second_directory = workspace.path().join("second");
    let first = create_bash_candidate(&first_directory)?;
    create_bash_candidate(&second_directory)?;
    let path = joined_path(&[first_directory.as_path(), second_directory.as_path()])?;

    let selected = resolve_bash_from_path(Some(path), workspace.path())?;

    anyhow::ensure!(
        selected.as_path()
            == Utf8Path::from_path(&first)
                .ok_or_else(|| { anyhow::anyhow!("fixture executable path should be UTF-8") })?,
        "resolution should choose the first PATH match"
    );
    Ok(())
}

/// Preserve absolute Unicode, spaced, and dollar-bearing path components.
#[test]
fn bash_resolution_keeps_unicode_space_and_dollar_paths() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let directory = workspace.path().join("Git Bash λ $ tools");
    let executable = create_bash_candidate(&directory)?;
    let path = joined_path(&[directory.as_path()])?;

    let selected = resolve_bash_from_path(Some(path), workspace.path())?;

    anyhow::ensure!(
        selected.as_path()
            == Utf8Path::from_path(&executable)
                .ok_or_else(|| { anyhow::anyhow!("fixture executable path should be UTF-8") })?,
        "resolution should preserve the selected Unicode path"
    );
    anyhow::ensure!(
        selected.as_path().is_absolute(),
        "selected Bash executable should be absolute"
    );
    Ok(())
}

/// Ignore executables outside the supplied `PATH`, including the current directory.
#[test]
fn bash_resolution_does_not_search_application_or_current_directories() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let current_directory = workspace.path().join("application");
    let selected_directory = workspace.path().join("selected-path");
    create_bash_candidate(&current_directory)?;
    test_support::fs::create_dir_all(&selected_directory)?;
    let path = joined_path(&[selected_directory.as_path()])?;

    let error = resolve_bash_from_path(Some(path), &current_directory)
        .expect_err("only PATH entries may supply the executable");

    anyhow::ensure!(
        error.to_string().contains("PATH contains no bash.exe"),
        "missing Bash diagnostic should identify the empty PATH lookup"
    );
    Ok(())
}

/// Reject an absent `PATH` and a list with no Bash executable.
#[rstest::rstest]
#[case::unset(None)]
#[case::empty(Some(OsString::new()))]
fn bash_resolution_reports_missing_runtime(#[case] path: Option<OsString>) {
    let workspace = tempfile::tempdir().expect("create lookup fixture");

    let error = resolve_bash_from_path(path, workspace.path())
        .expect_err("a missing Bash executable should fail resolution");

    assert!(error.to_string().contains("PATH contains no bash.exe"));
}

/// Pass the resolved executable identity unchanged to the injected probe.
#[test]
fn bash_preflight_probes_the_selected_absolute_path() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let directory = workspace.path().join("Git Bash");
    let executable = create_bash_candidate(&directory)?;
    let path = joined_path(&[directory.as_path()])?;
    let env = path_env(Some(path));
    let mut probed = None;

    let selected = resolve_bash_runtime_with(
        &BashRuntimeContext {
            is_windows: true,
            shell: RecipeShell::Bash,
            env: &env,
            current_dir: workspace.path(),
        },
        |selected_executable| {
            probed = Some(selected_executable.as_path().to_owned());
            Ok(BashProbeStatus::Available)
        },
    )?
    .expect("Windows Bash selection should produce a binding");

    let expected = Utf8PathBuf::from_path_buf(executable)
        .map_err(|_| anyhow::anyhow!("fixture executable path should be UTF-8"))?;
    anyhow::ensure!(
        selected.as_path() == expected,
        "preflight should return the selected executable"
    );
    anyhow::ensure!(
        probed.as_ref() == Some(&expected),
        "probe should receive the selected executable"
    );

    let mut graph = BuildGraph::default();
    graph.actions.insert(
        "run".into(),
        Action {
            recipe: Recipe::Command {
                command: StringOrList::String("echo exact".into()),
                isolation: CommandIsolation::Shared,
            },
            description: None,
            depfile: None,
            deps_format: None,
            pool: None,
            restat: false,
        },
    );
    let bundle = crate::ninja_gen::dyndep::generate_bundle_for_shell_with_bash(
        &graph,
        RecipeShell::Bash,
        Some(&selected),
    )?;
    let ninja_path = selected.as_path().as_str().replace('$', "$$");
    anyhow::ensure!(
        bundle
            .build_file()
            .contains(&format!("command = \"{ninja_path}\" -e -c \"echo exact\"")),
        "generated command should retain the preflighted executable"
    );
    Ok(())
}

/// Keep the first PATH match selected even when its probe fails.
#[test]
fn bash_preflight_does_not_fall_back_after_a_failed_probe() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let first_directory = workspace.path().join("first");
    let second_directory = workspace.path().join("second");
    let first = create_bash_candidate(&first_directory)?;
    create_bash_candidate(&second_directory)?;
    let path = joined_path(&[first_directory.as_path(), second_directory.as_path()])?;
    let env = path_env(Some(path));
    let mut probe_count = 0;

    let error = resolve_bash_runtime_with(
        &BashRuntimeContext {
            is_windows: true,
            shell: RecipeShell::Bash,
            env: &env,
            current_dir: workspace.path(),
        },
        |_| {
            probe_count += 1;
            Ok(BashProbeStatus::Failed {
                status: "exit status: 7".into(),
                output: "first runtime failed".into(),
            })
        },
    )
    .expect_err("a failed first match must not switch to a later executable");

    anyhow::ensure!(
        probe_count == 1,
        "only the first PATH match should be probed"
    );
    anyhow::ensure!(
        error.to_string().contains(&first.display().to_string()),
        "failed-probe diagnostic should name the selected executable"
    );
    anyhow::ensure!(
        error.to_string().contains("first runtime failed"),
        "failed-probe diagnostic should include captured output"
    );
    Ok(())
}

/// Keep non-Bash routes independent of `PATH` and Bash process probing.
#[test]
fn non_bash_selection_skips_path_lookup_and_probe() {
    let env = MockEnv::new();
    for shell in [RecipeShell::Posix, RecipeShell::PowerShell] {
        let result = resolve_bash_runtime_with(
            &BashRuntimeContext {
                is_windows: true,
                shell,
                env: &env,
                current_dir: Path::new("."),
            },
            |_| panic!("{shell:?} must not probe Bash"),
        );
        assert!(result.is_ok_and(|runtime| runtime.is_none()));
    }
}

/// Include the selected path, probe arguments, status, and bounded output on failure.
#[test]
fn failed_probe_reports_selected_executable_and_sanitized_output() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let executable = crate::recipe_shell::BashExecutable::from_absolute_path(
        Utf8PathBuf::from_path_buf(workspace.path().join("Git Bash/bin/bash.exe"))
            .map_err(|_| anyhow::anyhow!("fixture executable path should be UTF-8"))?,
    )?;

    let error = validate_bash_probe(
        &executable,
        Ok(BashProbeStatus::Failed {
            status: "exit status: 7".into(),
            output: describe_probe_output(b"version\n", b"bad\x1b[31m runtime"),
        }),
    )
    .expect_err("the failed version probe must be actionable");

    let rendered = format!("{error:#}");
    anyhow::ensure!(rendered.contains("Git Bash/bin/bash.exe"));
    anyhow::ensure!(rendered.contains("--version"));
    anyhow::ensure!(rendered.contains("exit status: 7"));
    anyhow::ensure!(rendered.contains("bad [31m runtime"));
    Ok(())
}

/// Preserve the selected path and OS source when starting the probe fails.
#[test]
fn failed_probe_launch_reports_selected_path_and_source() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let executable = crate::recipe_shell::BashExecutable::from_absolute_path(
        Utf8PathBuf::from_path_buf(workspace.path().join("Git Bash/bin/bash.exe"))
            .map_err(|_| anyhow::anyhow!("fixture executable path should be UTF-8"))?,
    )?;

    let error = validate_bash_probe(
        &executable,
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "cannot launch fixture",
        )),
    )
    .expect_err("launch failure should be actionable");

    let rendered = format!("{error:#}");
    anyhow::ensure!(rendered.contains("Git Bash/bin/bash.exe"));
    anyhow::ensure!(rendered.contains("--version"));
    anyhow::ensure!(rendered.contains("cannot launch fixture"));
    Ok(())
}

/// Bound captured output while consuming the complete child pipe.
#[test]
fn probe_output_capture_is_bounded() -> anyhow::Result<()> {
    let output = read_bounded_and_drain(Cursor::new(vec![b'x'; MAX_PROBE_OUTPUT_BYTES * 3]))?;

    anyhow::ensure!(
        output.len() == MAX_PROBE_OUTPUT_BYTES,
        "captured output should stop at the configured limit"
    );
    Ok(())
}

/// Replace control characters so child output stays on one diagnostic line.
#[test]
fn probe_output_sanitizes_control_characters() {
    assert_eq!(
        sanitized_output(b"first\nsecond\tthird\x1b[31m"),
        "first second third [31m"
    );
}
