//! Step definitions for progress-output scenarios that require fake Ninja output.

use crate::bdd::fixtures::TestWorld;
use crate::bdd::helpers::assertions::normalize_fluent_isolates;
use anyhow::{Context, Result, ensure};
use std::fs;
use std::path::{Path, PathBuf};

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
#[expect(
    clippy::unnecessary_wraps,
    reason = "the fallible signature must match the Unix variant so the shared call site needs no platform-specific handling"
)]
const fn make_script_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// Configuration for fake Ninja script generation.
struct FakeNinjaConfig<'a> {
    /// Lines to emit to stdout.
    stdout_lines: &'a [&'a str],
    /// Optional marker to emit to stderr for stream separation tests.
    stderr_marker: Option<&'a str>,
    /// Optional build artefact to create in the active workspace.
    artefact: Option<(&'a str, &'a str)>,
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
        if let Some((path, contents)) = config.artefact {
            script.push_str("echo ");
            script.push_str(contents);
            script.push_str(" > ");
            script.push_str(path);
            script.push_str("\r\n");
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
        if let Some((path, contents)) = config.artefact {
            script.push_str("printf '%s\\n' '");
            script.push_str(contents);
            script.push_str("' > '");
            script.push_str(path);
            script.push_str("'\n");
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

    world.track_env_var(
        netsuke::runner::NINJA_ENV.to_owned(),
        Some(script_path.as_os_str().to_owned()),
    );
    Ok(())
}

fn install_fake_ninja(world: &TestWorld, lines: &[&str]) -> Result<()> {
    install_fake_ninja_with_config(
        world,
        &FakeNinjaConfig {
            stdout_lines: lines,
            stderr_marker: None,
            artefact: None,
        },
    )
}

/// Bytes emitted before the newline in the oversized-status BDD fixture.
const OVERSIZED_STATUS_PREFIX_BYTES: usize = 513;

/// Build the distinctive unterminated prefix used by the oversized-status fixture.
fn oversized_status_prefix() -> String {
    "x".repeat(OVERSIZED_STATUS_PREFIX_BYTES)
}

/// Install a fake Ninja that emits an oversized raw line before a valid status line.
///
/// [`FakeNinjaConfig`] deliberately terminates every configured line, so this
/// fixture writes a raw prefix to exercise the parser's ignore-until-newline
/// state through the real CLI process boundary.
fn install_fake_ninja_with_oversized_status_line(world: &TestWorld) -> Result<()> {
    let root = workspace_root(world)?;
    let script_path = fake_ninja_path(&root);
    let prefix = oversized_status_prefix();
    let script = if cfg!(windows) {
        format!(
            "@echo off\r\n<nul set /p \"={prefix}\"\r\necho.\r\necho [1/2] cc -c resumed.c\r\nexit /B 0\r\n"
        )
    } else {
        format!("#!/bin/sh\nprintf '%s' '{prefix}'\nprintf '\n[1/2] cc -c resumed.c\n'\nexit 0\n")
    };
    fs::write(&script_path, script)
        .with_context(|| format!("write fake ninja script {}", script_path.display()))?;
    make_script_executable(&script_path)?;
    world.track_env_var(
        netsuke::runner::NINJA_ENV.to_owned(),
        Some(script_path.as_os_str().to_owned()),
    );
    Ok(())
}

#[rstest_bdd_macros::given("a fake ninja executable that emits task status lines")]
fn fake_ninja_emits_task_status_lines(world: &TestWorld) -> Result<()> {
    install_fake_ninja(world, &["[1/2] cc -c src/a.c", "[2/2] cc -c src/b.c"])
}

#[rstest_bdd_macros::given("a fake ninja executable that succeeds without output")]
fn fake_ninja_succeeds_without_output(world: &TestWorld) -> Result<()> {
    install_fake_ninja(world, &[])
}

#[rstest_bdd_macros::given(
    "a fake ninja executable that emits task status lines and builds hello.txt"
)]
fn fake_ninja_builds_documented_hello(world: &TestWorld) -> Result<()> {
    install_fake_ninja_with_config(
        world,
        &FakeNinjaConfig {
            stdout_lines: &["[1/1] echo Hello from Netsuke!"],
            stderr_marker: None,
            artefact: Some(("hello.txt", "Hello from Netsuke!")),
        },
    )
}

#[rstest_bdd_macros::given("a fake ninja executable that emits malformed task status lines")]
fn fake_ninja_emits_malformed_task_status_lines(world: &TestWorld) -> Result<()> {
    install_fake_ninja(world, &["[x/2] broken", "[2/] broken", "plain output only"])
}

#[rstest_bdd_macros::given(
    "a fake ninja executable that emits an oversized status line then a valid task status line"
)]
fn fake_ninja_emits_oversized_status_line(world: &TestWorld) -> Result<()> {
    install_fake_ninja_with_oversized_status_line(world)
}

#[rstest_bdd_macros::then("stdout should contain the oversized Ninja status prefix")]
fn stdout_contains_oversized_ninja_status_prefix(world: &TestWorld) -> Result<()> {
    let stdout = world
        .command_stdout
        .get()
        .context("no stdout captured for oversized Ninja status assertion")?;
    ensure!(
        stdout.contains(&oversized_status_prefix()),
        "stdout should retain the full oversized Ninja status prefix"
    );
    Ok(())
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
            artefact: None,
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
