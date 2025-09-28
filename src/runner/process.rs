use super::{BuildTargets, NinjaContent, NINJA_PROGRAM};
use crate::cli::Cli;
use anyhow::{Context, Result as AnyResult};
use ninja_env::NINJA_ENV;
use tempfile::{Builder, NamedTempFile};
use std::{
    env,
    fs,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
};
use tracing::info;

#[derive(Debug, Clone)]
pub struct CommandArg(String);
impl CommandArg {
    #[must_use]
    pub fn new(arg: String) -> Self {
        Self(arg)
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Public helpers for doctests only. This exposes internal helpers as a stable
// testing surface without exporting them in release builds.
#[doc(hidden)]
pub mod doc {
    pub use super::CommandArg;

    #[must_use]
    pub fn contains_sensitive_keyword(arg: &CommandArg) -> bool {
        super::contains_sensitive_keyword(arg)
    }
    #[must_use]
    pub fn is_sensitive_arg(arg: &CommandArg) -> bool {
        super::is_sensitive_arg(arg)
    }
    #[must_use]
    pub fn redact_argument(arg: &CommandArg) -> CommandArg {
        super::redact_argument(arg)
    }
    #[must_use]
    pub fn redact_sensitive_args(args: &[CommandArg]) -> Vec<CommandArg> {
        super::redact_sensitive_args(args)
    }

    pub fn create_temp_ninja_file(
        content: &super::NinjaContent,
    ) -> super::AnyResult<super::NamedTempFile> {
        super::create_temp_ninja_file(content)
    }

    pub fn write_ninja_file(
        path: &super::Path,
        content: &super::NinjaContent,
    ) -> super::AnyResult<()> {
        super::write_ninja_file(path, content)
    }
}

/// Check if `arg` contains a sensitive keyword.
///
/// # Examples
/// ```
/// # use netsuke::runner::doc::{CommandArg, contains_sensitive_keyword};
/// assert!(contains_sensitive_keyword(&CommandArg::new("token=abc".into())));
/// assert!(!contains_sensitive_keyword(&CommandArg::new("path=/tmp".into())));
/// ```
pub(crate) fn contains_sensitive_keyword(arg: &CommandArg) -> bool {
    let lower = arg.as_str().to_lowercase();
    lower.contains("password") || lower.contains("token") || lower.contains("secret")
}

/// Determine whether the argument should be redacted.
/// Determine whether the argument should be redacted.
///
/// # Examples
/// ```
/// # use netsuke::runner::doc::{CommandArg, is_sensitive_arg};
/// assert!(is_sensitive_arg(&CommandArg::new("password=123".into())));
/// assert!(!is_sensitive_arg(&CommandArg::new("file=readme".into())));
/// ```
pub(crate) fn is_sensitive_arg(arg: &CommandArg) -> bool {
    contains_sensitive_keyword(arg)
}

/// Redact sensitive information in a single argument.
///
/// Sensitive values are replaced with `***REDACTED***`, preserving keys.
///
/// # Examples
/// ```
/// # use netsuke::runner::doc::{CommandArg, redact_argument};
/// let arg = CommandArg::new("token=abc".into());
/// assert_eq!(redact_argument(&arg).as_str(), "token=***REDACTED***");
/// let arg = CommandArg::new("path=/tmp".into());
/// assert_eq!(redact_argument(&arg).as_str(), "path=/tmp");
/// ```
pub(crate) fn redact_argument(arg: &CommandArg) -> CommandArg {
    if is_sensitive_arg(arg) {
        let redacted = arg.as_str().split_once('=').map_or_else(
            || "***REDACTED***".to_string(),
            |(key, _)| format!("{key}=***REDACTED***"),
        );
        CommandArg::new(redacted)
    } else {
        arg.clone()
    }
}

/// Redact sensitive information from all `args`.
///
/// # Examples
/// ```
/// # use netsuke::runner::doc::{CommandArg, redact_sensitive_args};
/// let args = vec![
///     CommandArg::new("ninja".into()),
///     CommandArg::new("token=abc".into()),
/// ];
/// let redacted = redact_sensitive_args(&args);
/// assert_eq!(redacted[1].as_str(), "token=***REDACTED***");
/// ```
pub(crate) fn redact_sensitive_args(args: &[CommandArg]) -> Vec<CommandArg> {
    args.iter().map(redact_argument).collect()
}

/// Create a temporary Ninja file on disk containing `content`.
///
/// # Errors
///
/// Returns an error if the file cannot be created or written.
///
/// # Examples
/// ```ignore
/// use netsuke::runner::doc::create_temp_ninja_file;
/// use netsuke::runner::NinjaContent;
/// let tmp = create_temp_ninja_file(&NinjaContent::new("".into())).unwrap();
/// assert!(tmp.path().to_string_lossy().ends_with(".ninja"));
/// ```
pub(super) fn create_temp_ninja_file(content: &NinjaContent) -> AnyResult<NamedTempFile> {
    let tmp = Builder::new()
        .prefix("netsuke.")
        .suffix(".ninja")
        .tempfile()
        .context("create temp file")?;
    write_ninja_file(tmp.path(), content)?;
    Ok(tmp)
}

/// Write `content` to `path` and log the file's location.
///
/// # Errors
///
/// Returns an error if the file cannot be written.
///
/// # Examples
/// ```ignore
/// use std::path::Path;
/// use netsuke::runner::doc::write_ninja_file;
/// use netsuke::runner::NinjaContent;
/// let content = NinjaContent::new("rule cc\n".to_string());
/// write_ninja_file(Path::new("out.ninja"), &content).unwrap();
/// ```
pub(super) fn write_ninja_file(path: &Path, content: &NinjaContent) -> AnyResult<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create parent directory {}", parent.display()))?;
    }
    fs::write(path, content.as_str())
        .with_context(|| format!("failed to write Ninja file to {}", path.display()))?;
    info!("Generated Ninja file at {}", path.display());
    Ok(())
}

/// Determine which Ninja executable to invoke.
#[must_use]
pub(super) fn resolve_ninja_program() -> PathBuf {
    env::var_os(NINJA_ENV).map_or_else(|| PathBuf::from(NINJA_PROGRAM), PathBuf::from)
}

/// Invoke the Ninja executable with the provided CLI settings.
///
/// The function forwards the job count and working directory to Ninja,
/// specifies the temporary build file, and streams its standard output and
/// error back to the user.
///
/// # Errors
///
/// Returns an [`io::Error`] if the Ninja process fails to spawn or reports a
/// non-zero exit status.
///
/// # Panics
///
/// Panics if the child's output streams cannot be captured.
pub fn run_ninja(
    program: &Path,
    cli: &Cli,
    build_file: &Path,
    targets: &BuildTargets<'_>,
) -> io::Result<()> {
    let mut cmd = Command::new(program);
    if let Some(dir) = &cli.directory {
        let dir = fs::canonicalize(dir)?;
        cmd.current_dir(dir);
    }
    if let Some(jobs) = cli.jobs {
        cmd.arg("-j").arg(jobs.to_string());
    }
    let build_file_path = build_file
        .canonicalize()
        .unwrap_or_else(|_| build_file.to_path_buf());
    cmd.arg("-f").arg(&build_file_path);
    cmd.args(targets.as_slice());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let program = cmd.get_program().to_string_lossy().into_owned();
    let args: Vec<CommandArg> = cmd
        .get_args()
        .map(|a| CommandArg::new(a.to_string_lossy().into_owned()))
        .collect();
    let redacted_args = redact_sensitive_args(&args);
    let arg_strings: Vec<&str> = redacted_args.iter().map(CommandArg::as_str).collect();
    info!("Running command: {} {}", program, arg_strings.join(" "));

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().expect("child stdout");
    let stderr = child.stderr.take().expect("child stderr");

    let out_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut handle = io::stdout();
        for line in reader.lines().map_while(Result::ok) {
            let _ = writeln!(handle, "{line}");
        }
    });
    let err_handle = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut handle = io::stderr();
        for line in reader.lines().map_while(Result::ok) {
            let _ = writeln!(handle, "{line}");
        }
    });

    let status = child.wait()?;
    let _ = out_handle.join();
    let _ = err_handle.join();

    if status.success() {
        Ok(())
    } else {
        #[expect(
            clippy::io_other_error,
            reason = "use explicit error kind for compatibility with older Rust"
        )]
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("ninja exited with {status}"),
        ))
    }
}
