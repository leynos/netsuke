//! Test the Unix fake-Ninja factories owned by `check_ninja`.
//!
//! This private test sibling keeps the public helper module below Whitaker's
//! 400-line cap. It is compiled only for the Unix test configuration and may
//! call only the factories exported by its owning `check_ninja` module.

use anyhow::{Context, Result};
use rstest::rstest;
use std::path::Path;
use std::process::Command;

use super::{ToolName, fake_ninja_expect_tool_with_jobs};

#[rstest]
#[case(
    &["-f", "build.ninja", "-C", "/path/to/build", "-t", "clean"],
    true,
    "correct -C value"
)]
#[case(
    &["-f", "build.ninja", "-C", "/wrong/path", "-t", "clean"],
    false,
    "wrong -C value"
)]
#[case(&["-f", "build.ninja", "-t", "clean"], false, "missing -C flag")]
fn fake_ninja_validates_directory_flag(
    #[case] args: &[&str],
    #[case] should_succeed: bool,
    #[case] description: &str,
) -> Result<()> {
    let (dir, ninja_path) = fake_ninja_expect_tool_with_jobs(
        ToolName::new("clean"),
        None,
        Some(Path::new("/path/to/build")),
    )?;

    let status = Command::new(&ninja_path)
        .args(args)
        .current_dir(dir.path())
        .status()
        .context("execute fake ninja")?;

    anyhow::ensure!(
        status.success() == should_succeed,
        "unexpected fake Ninja result for {description}"
    );

    Ok(())
}
