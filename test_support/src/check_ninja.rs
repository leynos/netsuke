//! Helpers for validating build file paths and tool invocations via fake Ninja binaries.

use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

/// Make a script file executable on Unix platforms.
#[cfg(unix)]
fn make_script_executable(path: &PathBuf, context: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)
        .with_context(|| format!("{context}: read metadata {}", path.display()))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms)
        .with_context(|| format!("{context}: set permissions {}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn make_script_executable(_path: &PathBuf, _context: &str) -> Result<()> {
    Ok(())
}

/// Create a fake Ninja that validates the build file path provided via `-f`.
///
/// The script exits with status `1` if the file is missing or not a regular
/// file, otherwise `0`.
pub fn fake_ninja_check_build_file() -> Result<(TempDir, PathBuf)> {
    let dir = TempDir::new().context("fake_ninja_check_build_file: create temp dir")?;
    let path = dir.path().join("ninja");
    let mut file = File::create(&path).with_context(|| {
        format!(
            "fake_ninja_check_build_file: create script {}",
            path.display()
        )
    })?;
    writeln!(
        file,
        concat!(
            "#!/bin/sh\n",
            "if [ \"$1\" = \"-f\" ] && [ ! -f \"$2\" ]; then\n",
            "  echo 'missing build file: $2' >&2\n",
            "  exit 1\n",
            "fi\n",
            "exit 0"
        ),
    )
    .with_context(|| {
        format!(
            "fake_ninja_check_build_file: write script {}",
            path.display()
        )
    })?;
    make_script_executable(&path, "fake_ninja_check_build_file")?;
    Ok((dir, path))
}

/// Create a fake Ninja that validates `-t <tool>` was invoked with the expected tool name.
///
/// The script scans command-line arguments for `-t <tool>` and exits with status `0`
/// if the expected tool is found, otherwise `1`.
///
/// # Arguments
///
/// * `expected_tool` - The tool name that should follow `-t` (e.g., `"clean"`)
///
/// # Example
///
/// ```rust,ignore
/// use test_support::check_ninja::fake_ninja_expect_tool;
///
/// let (dir, ninja_path) = fake_ninja_expect_tool("clean")?;
/// // ninja_path will succeed only when invoked with `-t clean`
/// ```
#[cfg(unix)]
pub fn fake_ninja_expect_tool(expected_tool: &str) -> Result<(TempDir, PathBuf)> {
    let dir = TempDir::new().context("fake_ninja_expect_tool: create temp dir")?;
    let path = dir.path().join("ninja");
    let mut file = File::create(&path)
        .with_context(|| format!("fake_ninja_expect_tool: create script {}", path.display()))?;
    // Shell script that validates:
    // 1. `-t <tool>` is present with the expected tool name
    // 2. `-f <file>` is present (build file argument)
    writeln!(
        file,
        concat!(
            "#!/bin/sh\n",
            "expected=\"{expected}\"\n",
            "found_tool=0\n",
            "found_file=0\n",
            "prev=\"\"\n",
            "for arg in \"$@\"; do\n",
            "  if [ \"$prev\" = \"-t\" ] && [ \"$arg\" = \"$expected\" ]; then\n",
            "    found_tool=1\n",
            "  fi\n",
            "  if [ \"$prev\" = \"-f\" ]; then\n",
            "    found_file=1\n",
            "  fi\n",
            "  prev=\"$arg\"\n",
            "done\n",
            "if [ $found_tool -eq 0 ]; then\n",
            "  echo \"expected -t $expected but did not find it\" >&2\n",
            "  exit 1\n",
            "fi\n",
            "if [ $found_file -eq 0 ]; then\n",
            "  echo \"expected -f <build_file> but did not find it\" >&2\n",
            "  exit 1\n",
            "fi\n",
            "exit 0"
        ),
        expected = expected_tool
    )
    .with_context(|| format!("fake_ninja_expect_tool: write script {}", path.display()))?;
    make_script_executable(&path, "fake_ninja_expect_tool")?;
    Ok((dir, path))
}
