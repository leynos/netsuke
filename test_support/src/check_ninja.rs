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

/// Represents a Ninja command-line flag and its associated shell variable name.
#[cfg(unix)]
#[derive(Debug, Clone, Copy)]
struct ShellFlag {
    flag: &'static str,
    var_name: &'static str,
}

#[cfg(unix)]
impl ShellFlag {
    const JOBS: Self = Self {
        flag: "-j",
        var_name: "jobs",
    };

    const DIRECTORY: Self = Self {
        flag: "-C",
        var_name: "dir",
    };

    fn flag(&self) -> &str {
        self.flag
    }

    fn var_name(&self) -> &str {
        self.var_name
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
    fake_ninja_expect_tool_with_jobs(expected_tool, None, None)
}

/// Builds all three shell script snippets for validating an optional flag.
///
/// Returns a tuple of (init, loop_check, validation) strings for the given flag.
#[cfg(unix)]
fn build_flag_validation(shell_flag: ShellFlag, expected_value: &str) -> (String, String, String) {
    let flag = shell_flag.flag();
    let var_name = shell_flag.var_name();
    let init = format!("expected_{var_name}=\"{expected_value}\"\nfound_{var_name}=0\n");
    let loop_check = format!(
        concat!(
            "  if [ \"$prev\" = \"{flag}\" ] && [ \"$arg\" = \"$expected_{var}\" ]; then\n",
            "    found_{var}=1\n",
            "  fi\n",
        ),
        flag = flag,
        var = var_name
    );
    let validation = format!(
        concat!(
            "if [ $found_{var} -eq 0 ]; then\n",
            "  echo \"expected {flag} $expected_{var} but did not find it\" >&2\n",
            "  exit 1\n",
            "fi\n",
        ),
        flag = flag,
        var = var_name
    );
    (init, loop_check, validation)
}

/// Shell script template for validating ninja tool invocation.
#[cfg(unix)]
const TOOL_VALIDATION_TEMPLATE: &str = concat!(
    "#!/bin/sh\n",
    "expected=\"{expected}\"\n",
    "{flag_inits}",
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
    "{flag_loop_checks}",
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
    "{flag_validations}",
    "exit 0\n"
);

/// Builds the shell script content for validating ninja tool invocation.
#[cfg(unix)]
fn build_tool_validation_script(
    expected_tool: ToolName,
    expected_jobs: Option<u32>,
    expected_directory: Option<&Path>,
) -> String {
    let (jobs_init, jobs_loop, jobs_valid) = expected_jobs
        .map(|j| build_flag_validation(ShellFlag::JOBS, &j.to_string()))
        .unwrap_or_default();
    let (dir_init, dir_loop, dir_valid) = expected_directory
        .and_then(|p| p.to_str())
        .map(|d| build_flag_validation(ShellFlag::DIRECTORY, d))
        .unwrap_or_default();

    TOOL_VALIDATION_TEMPLATE
        .replace("{expected}", expected_tool.as_str())
        .replace("{flag_inits}", &format!("{jobs_init}{dir_init}"))
        .replace("{flag_loop_checks}", &format!("{jobs_loop}{dir_loop}"))
        .replace("{flag_validations}", &format!("{jobs_valid}{dir_valid}"))
}

/// Create a fake Ninja that validates `-t <tool>` and optionally `-j <jobs>` and `-C <dir>`.
///
/// The script scans command-line arguments for `-t <tool>`, `-f <file>`, and
/// optionally `-j <jobs>` and `-C <directory>`. It exits with status `0` if all
/// expected arguments are found, otherwise `1`.
///
/// # Arguments
///
/// * `expected_tool` - The tool name that should follow `-t` (e.g., `"clean"`)
/// * `expected_jobs` - Optional job count that should follow `-j`
/// * `expected_directory` - Optional working directory that should follow `-C`
///
/// # Example
///
/// ```rust,ignore
/// use std::path::Path;
/// use test_support::check_ninja::{fake_ninja_expect_tool_with_jobs, ToolName};
///
/// let (dir, ninja_path) = fake_ninja_expect_tool_with_jobs(
///     ToolName::new("clean"),
///     Some(4),
///     Some(Path::new("/path/to/build")),
/// )?;
/// // ninja_path will succeed only when invoked with `-t clean -j 4 -C /path/to/build`
/// ```
#[cfg(unix)]
pub fn fake_ninja_expect_tool_with_jobs(
    expected_tool: ToolName,
    expected_jobs: Option<u32>,
    expected_directory: Option<&Path>,
) -> Result<(TempDir, PathBuf)> {
    let dir = TempDir::new().context("fake_ninja_expect_tool_with_jobs: create temp dir")?;
    let path = dir.path().join("ninja");
    let mut file = File::create(&path).with_context(|| {
        format!(
            "fake_ninja_expect_tool_with_jobs: create script {}",
            path.display()
        )
    })?;
    let script_content =
        build_tool_validation_script(expected_tool, expected_jobs, expected_directory);
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
    _expected_directory: Option<&Path>,
) -> Result<(TempDir, PathBuf)> {
    anyhow::bail!("fake_ninja_expect_tool_with_jobs is only supported on Unix platforms")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that the fake ninja script validates `-C <directory>` correctly.
    #[cfg(unix)]
    #[rstest::rstest]
    #[case(&["-f", "build.ninja", "-C", "/path/to/build", "-t", "clean"], true, "correct -C value")]
    #[case(&["-f", "build.ninja", "-C", "/wrong/path", "-t", "clean"], false, "wrong -C value")]
    #[case(&["-f", "build.ninja", "-t", "clean"], false, "missing -C flag")]
    fn fake_ninja_validates_directory_flag(
        #[case] args: &[&str],
        #[case] should_succeed: bool,
        #[case] description: &str,
    ) -> Result<()> {
        use anyhow::Context;
        use std::process::Command;

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

        assert_eq!(status.success(), should_succeed, "{description}");

        Ok(())
    }
}
