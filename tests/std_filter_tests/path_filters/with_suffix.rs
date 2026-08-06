//! Integration coverage for the `with_suffix` path filter.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::ErrorKind;
use rstest::{fixture, rstest};
use serde_json::json;

use super::{FilterErrorTest, Workspace, fallible, test_filter_error, with_filter_env};

/// Yield a fresh, isolated filter workspace.
///
/// rstest invokes this per test case, so each test owns its own `TempDir` and
/// no fixture state leaks between them.
#[fixture]
fn workspace() -> Result<Workspace> {
    fallible::filter_workspace()
}

#[rstest]
fn with_suffix_filter(workspace: Result<Workspace>) -> Result<()> {
    with_filter_env(workspace?, |root, env| {
        let file = root.join("file.tar.gz");
        Dir::open_ambient_dir(root, ambient_authority())
            .context("open workspace root")?
            .write("file.tar.gz", b"data")
            .context("write archive fixture")?;
        let first = fallible::render(env, "suffix", "{{ path | with_suffix('.log') }}", &file)
            .context("render with_suffix(.log)")?;
        ensure!(
            first == root.join("file.tar.log").as_str(),
            "expected '.log' suffix to replace final component but rendered {first}"
        );
        let second = fallible::render(
            env,
            "suffix_alt",
            "{{ path | with_suffix('.zip', 2) }}",
            &file,
        )
        .context("render with_suffix(.zip, 2)")?;
        ensure!(
            second == root.join("file.zip").as_str(),
            "expected two extensions to be replaced but rendered {second}"
        );
        let third = fallible::render(
            env,
            "suffix_count_zero",
            "{{ path | with_suffix('.bak', 0) }}",
            &file,
        )
        .context("render with_suffix(.bak, 0)")?;
        ensure!(
            third == root.join("file.tar.gz.bak").as_str(),
            "expected zero count to append suffix but rendered {third}"
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn with_suffix_filter_without_separator(workspace: Result<Workspace>) -> Result<()> {
    with_filter_env(workspace?, |root, env| {
        let file = root.join("file");
        let output = fallible::render(
            env,
            "suffix_plain",
            "{{ path | with_suffix('.log') }}",
            &file,
        )
        .context("render with_suffix on filename without separator")?;
        ensure!(
            output == root.join("file.log").as_str(),
            "expected '.log' to be appended but rendered {output}"
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn with_suffix_filter_empty_separator(workspace: Result<Workspace>) -> Result<()> {
    test_filter_error(
        workspace?,
        FilterErrorTest {
            name: "suffix_empty_sep",
            template: "{{ path | with_suffix('.log', 1, '') }}",
            context: json!({
                "path": "file.tar.gz",
            }),
            error_kind: ErrorKind::InvalidOperation,
            error_contains: "non-empty separator",
            env_setup: None,
        },
    )
}

#[rstest]
fn with_suffix_filter_excessive_count(workspace: Result<Workspace>) -> Result<()> {
    with_filter_env(workspace?, |root, env| {
        let file = root.join("file.tar.gz");
        let output = fallible::render(
            env,
            "suffix_excessive",
            "{{ path | with_suffix('.bak', 5) }}",
            &file,
        )
        .context("render with_suffix(.bak, 5)")?;
        ensure!(
            output == root.join("file.bak").as_str(),
            "expected excessive count to collapse extensions but rendered {output}"
        );
        Ok(())
    })?;
    Ok(())
}
