//! Step definitions for progress-output scenarios that require fake Ninja output.

use crate::bdd::fixtures::TestWorld;
use crate::bdd::helpers::assertions::normalize_fluent_isolates;
use anyhow::{Context, Result, ensure};
use std::fs;
use std::path::{Path, PathBuf};
use test_support::env::{SystemEnv, override_ninja_env};

fn workspace_root(world: &TestWorld) -> Result<PathBuf> {
    let temp = world.temp_dir.borrow();
    let dir = temp.as_ref().context("temp dir has not been initialised")?;
    Ok(dir.path().to_path_buf())
}

#[cfg(unix)]
fn make_script_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .with_context(|| format!("read metadata for {}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("set executable bit for {}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn make_script_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// Configuration for fake Ninja script generation.
struct FakeNinjaConfig<'a> {
    /// Lines to emit to stdout.
    stdout_lines: &'a [&'a str],
    /// Optional marker to emit to stderr for stream separation tests.
    stderr_marker: Option<&'a str>,
}

fn build_fake_ninja_script(config: &FakeNinjaConfig<'_>) -> String {
    if cfg!(windows) {
        let mut script = String::from("@echo off\r\n");
        for line in config.stdout_lines {
            script.push_str("echo ");
            script.push_str(line);
            script.push_str("\r\n");
        }
        if let Some(marker) = config.stderr_marker {
            script.push_str("echo ");
            script.push_str(marker);
            script.push_str(" 1>&2\r\n");
        }
        script.push_str("exit /B 0\r\n");
        script
    } else {
        let mut script = String::from(
            "#!/bin/sh\nwhile IFS= read -r line; do\n  printf '%s\\n' \"$line\"\ndone <<'NETSUKE_STATUS'\n",
        );
        for line in config.stdout_lines {
            script.push_str(line);
            script.push('\n');
        }
        script.push_str("NETSUKE_STATUS\n");
        if let Some(marker) = config.stderr_marker {
            script.push_str("printf '%s\\n' '");
            script.push_str(marker);
            script.push_str("' >&2\n");
        }
        script.push_str("exit 0\n");
        script
    }
}

fn fake_ninja_path(root: &Path) -> PathBuf {
    if cfg!(windows) {
        return root.join("fake-ninja-progress.cmd");
    }
    root.join("fake-ninja-progress")
}

fn install_fake_ninja_with_config(world: &TestWorld, config: &FakeNinjaConfig<'_>) -> Result<()> {
    let root = workspace_root(world)?;
    let script_path = fake_ninja_path(&root);
    let script = build_fake_ninja_script(config);
    fs::write(&script_path, script)
        .with_context(|| format!("write fake ninja script {}", script_path.display()))?;
    make_script_executable(&script_path)?;

    let env = SystemEnv::new();
    // Drop any existing guard first so its environment override is restored
    // before installing a replacement for this scenario.
    world.ninja_env_guard.borrow_mut().take();
    let script_path_os = script_path.as_os_str().to_owned();
    // Get the original value before setting so we can track it.
    // The lock is acquired inside override_ninja_env, and the guard handles restoration.
    let previous = {
        let _lock = test_support::env_lock::EnvLock::acquire();
        std::env::var_os(ninja_env::NINJA_ENV)
    };
    *world.ninja_env_guard.borrow_mut() = Some(override_ninja_env(&env, &script_path));
    world.track_env_var(
        ninja_env::NINJA_ENV.to_owned(),
        previous,
        Some(script_path_os),
    );
    Ok(())
}

fn install_fake_ninja(world: &TestWorld, lines: &[&str]) -> Result<()> {
    install_fake_ninja_with_config(
        world,
        &FakeNinjaConfig {
            stdout_lines: lines,
            stderr_marker: None,
        },
    )
}

#[rstest_bdd_macros::given("a fake ninja executable that emits task status lines")]
fn fake_ninja_emits_task_status_lines(world: &TestWorld) -> Result<()> {
    install_fake_ninja(world, &["[1/2] cc -c src/a.c", "[2/2] cc -c src/b.c"])
}

#[rstest_bdd_macros::given("a fake ninja executable that emits malformed task status lines")]
fn fake_ninja_emits_malformed_task_status_lines(world: &TestWorld) -> Result<()> {
    install_fake_ninja(world, &["[x/2] broken", "[2/] broken", "plain output only"])
}

#[rstest_bdd_macros::given("a fake ninja executable that emits stdout output")]
fn fake_ninja_emits_stdout_output(world: &TestWorld) -> Result<()> {
    install_fake_ninja_with_config(
        world,
        &FakeNinjaConfig {
            stdout_lines: &[
                "[1/2] cc -c src/a.c",
                "NINJA_STDOUT_MARKER_LINE_1",
                "[2/2] cc -c src/b.c",
                "NINJA_STDOUT_MARKER_LINE_2",
            ],
            stderr_marker: Some("NINJA_STDERR_MARKER"),
        },
    )
}

#[rstest_bdd_macros::then("stderr lines containing {pattern} should all start with {prefix}")]
fn stderr_lines_containing_pattern_should_start_with_prefix(
    world: &TestWorld,
    pattern: &str,
    prefix: &str,
) -> Result<()> {
    let stderr = world
        .command_stderr
        .get()
        .context("no stderr captured for progress output assertion")?;
    let normalized_stderr = normalize_fluent_isolates(&stderr);
    let normalized_pattern = normalize_fluent_isolates(pattern.trim_matches('"'));
    let normalized_prefix = normalize_fluent_isolates(prefix.trim_matches('"'));
    let matching_lines = normalized_stderr
        .lines()
        .filter(|line| line.contains(&normalized_pattern))
        .collect::<Vec<_>>();

    ensure!(
        !matching_lines.is_empty(),
        "no normalized stderr lines contained pattern '{normalized_pattern}'"
    );

    for line in matching_lines {
        ensure!(
            line.starts_with(&normalized_prefix),
            "expected normalized stderr line '{line}' to start with '{normalized_prefix}'"
        );
    }

    Ok(())
}
