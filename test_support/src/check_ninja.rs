//! Helpers for validating build file paths and tool invocations via fake Ninja binaries.

use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Represents a Ninja tool name (e.g., "clean", "compdb").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolName(&'static str);

impl ToolName {
    /// Create a new tool name.
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// Get the tool name as a string slice.
    pub fn as_str(&self) -> &str {
        self.0
    }
}

impl From<&'static str> for ToolName {
    fn from(name: &'static str) -> Self {
        Self(name)
    }
}

/// Make a script file executable on Unix platforms.
#[cfg(unix)]
fn make_script_executable(path: &Path, context: &str) -> Result<()> {
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
fn make_script_executable(_path: &Path, _context: &str) -> Result<()> {
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
/// if the expected tool is found, otherwise `1`. It also validates that `-f <file>`
/// is present.
///
/// # Arguments
///
/// * `expected_tool` - The tool name that should follow `-t` (e.g., `"clean"`)
///
/// # Example
///
/// ```rust,ignore
/// use test_support::check_ninja::{fake_ninja_expect_tool, ToolName};
///
/// let (dir, ninja_path) = fake_ninja_expect_tool(ToolName::new("clean"))?;
/// // ninja_path will succeed only when invoked with `-t clean`
/// ```
#[cfg(unix)]
pub fn fake_ninja_expect_tool(expected_tool: ToolName) -> Result<(TempDir, PathBuf)> {
    fake_ninja_expect_tool_with_jobs(expected_tool, None)
}

/// Builds the shell script content for validating ninja tool invocation.
#[cfg(unix)]
fn build_tool_validation_script(expected_tool: ToolName, expected_jobs: Option<u32>) -> String {
    let expected = expected_tool.as_str();
    let jobs_check = if let Some(jobs) = expected_jobs {
        format!(
            concat!("expected_jobs=\"{jobs}\"\n", "found_jobs=0\n",),
            jobs = jobs
        )
    } else {
        String::new()
    };

    let jobs_loop_check = if expected_jobs.is_some() {
        concat!(
            "  if [ \"$prev\" = \"-j\" ] && [ \"$arg\" = \"$expected_jobs\" ]; then\n",
            "    found_jobs=1\n",
            "  fi\n",
        )
    } else {
        ""
    };

    let jobs_validation = if expected_jobs.is_some() {
        concat!(
            "if [ $found_jobs -eq 0 ]; then\n",
            "  echo \"expected -j $expected_jobs but did not find it\" >&2\n",
            "  exit 1\n",
            "fi\n",
        )
    } else {
        ""
    };

    format!(
        concat!(
            "#!/bin/sh\n",
            "expected=\"{expected}\"\n",
            "{jobs_check}",
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
            "{jobs_loop_check}",
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
            "{jobs_validation}",
            "exit 0\n"
        ),
        expected = expected,
        jobs_check = jobs_check,
        jobs_loop_check = jobs_loop_check,
        jobs_validation = jobs_validation,
    )
}

/// Create a fake Ninja that validates `-t <tool>` and optionally `-j <jobs>`.
///
/// The script scans command-line arguments for `-t <tool>`, `-f <file>`, and
/// optionally `-j <jobs>`. It exits with status `0` if all expected arguments
/// are found, otherwise `1`.
///
/// # Arguments
///
/// * `expected_tool` - The tool name that should follow `-t` (e.g., `"clean"`)
/// * `expected_jobs` - Optional job count that should follow `-j`
///
/// # Example
///
/// ```rust,ignore
/// use test_support::check_ninja::{fake_ninja_expect_tool_with_jobs, ToolName};
///
/// let (dir, ninja_path) = fake_ninja_expect_tool_with_jobs(ToolName::new("clean"), Some(4))?;
/// // ninja_path will succeed only when invoked with `-t clean -j 4`
/// ```
#[cfg(unix)]
pub fn fake_ninja_expect_tool_with_jobs(
    expected_tool: ToolName,
    expected_jobs: Option<u32>,
) -> Result<(TempDir, PathBuf)> {
    let dir = TempDir::new().context("fake_ninja_expect_tool_with_jobs: create temp dir")?;
    let path = dir.path().join("ninja");
    let mut file = File::create(&path).with_context(|| {
        format!(
            "fake_ninja_expect_tool_with_jobs: create script {}",
            path.display()
        )
    })?;
    let script_content = build_tool_validation_script(expected_tool, expected_jobs);
    write!(file, "{}", script_content).with_context(|| {
        format!(
            "fake_ninja_expect_tool_with_jobs: write script {}",
            path.display()
        )
    })?;
    make_script_executable(&path, "fake_ninja_expect_tool_with_jobs")?;
    Ok((dir, path))
}

/// Stub for non-Unix platforms that returns an error.
#[cfg(not(unix))]
pub fn fake_ninja_expect_tool(_expected_tool: ToolName) -> Result<(TempDir, PathBuf)> {
    anyhow::bail!("fake_ninja_expect_tool is only supported on Unix platforms")
}

/// Stub for non-Unix platforms that returns an error.
#[cfg(not(unix))]
pub fn fake_ninja_expect_tool_with_jobs(
    _expected_tool: ToolName,
    _expected_jobs: Option<u32>,
) -> Result<(TempDir, PathBuf)> {
    anyhow::bail!("fake_ninja_expect_tool_with_jobs is only supported on Unix platforms")
}
