//! Process helpers for Ninja file lifecycle, argument redaction, and subprocess I/O.
//! Internal to `runner`; public API is defined in `runner.rs`.
use super::{BuildTargets, NINJA_PROGRAM, NinjaContent};
use crate::cli::Cli;
use anyhow::{Context, Result as AnyResult, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs as cap_fs};
use ninja_env::NINJA_ENV;
use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
};
use tempfile::{Builder, NamedTempFile};
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
#[cfg(doctest)]
pub mod doc {
    pub use super::{
        CommandArg, contains_sensitive_keyword, create_temp_ninja_file, is_sensitive_arg,
        redact_argument, redact_sensitive_args, resolve_ninja_program, resolve_ninja_program_utf8,
        write_ninja_file, write_ninja_file_utf8,
    };
}

fn is_sensitive_key(key: &str) -> bool {
    const SENSITIVE_KEYS: [&str; 7] = [
        "password",
        "token",
        "secret",
        "api_key",
        "apikey",
        "auth",
        "authorization",
    ];
    SENSITIVE_KEYS
        .iter()
        .any(|candidate| key.eq_ignore_ascii_case(candidate))
}

/// Check if `arg` contains a sensitive keyword.
///
/// # Examples
/// ```ignore
/// # use netsuke::runner::doc::{CommandArg, contains_sensitive_keyword};
/// assert!(contains_sensitive_keyword(&CommandArg::new("token=abc".into())));
/// assert!(!contains_sensitive_keyword(&CommandArg::new("path=/tmp".into())));
/// ```
#[must_use]
pub fn contains_sensitive_keyword(arg: &CommandArg) -> bool {
    arg.as_str()
        .split_once('=')
        .is_some_and(|(key, _)| is_sensitive_key(key.trim()))
}

/// Determine whether the argument should be redacted.
///
/// # Examples
/// ```ignore
/// # use netsuke::runner::doc::{CommandArg, is_sensitive_arg};
/// assert!(is_sensitive_arg(&CommandArg::new("password=123".into())));
/// assert!(!is_sensitive_arg(&CommandArg::new("file=readme".into())));
/// ```
#[must_use]
pub fn is_sensitive_arg(arg: &CommandArg) -> bool {
    contains_sensitive_keyword(arg)
}

/// Redact sensitive information in a single argument.
///
/// Sensitive values are replaced with `***REDACTED***`, preserving keys.
///
/// # Examples
/// ```ignore
/// # use netsuke::runner::doc::{CommandArg, redact_argument};
/// let arg = CommandArg::new("token=abc".into());
/// assert_eq!(redact_argument(&arg).as_str(), "token=***REDACTED***");
/// let arg = CommandArg::new("path=/tmp".into());
/// assert_eq!(redact_argument(&arg).as_str(), "path=/tmp");
/// ```
#[must_use]
pub fn redact_argument(arg: &CommandArg) -> CommandArg {
    if is_sensitive_arg(arg) {
        if let Some((key, _)) = arg.as_str().split_once('=') {
            let trimmed = key.trim();
            return CommandArg::new(format!("{trimmed}=***REDACTED***"));
        }
        return CommandArg::new(String::from("***REDACTED***"));
    }
    arg.clone()
}

/// Redact sensitive information from all `args`.
///
/// # Examples
/// ```ignore
/// # use netsuke::runner::doc::{CommandArg, redact_sensitive_args};
/// let args = vec![
///     CommandArg::new("ninja".into()),
///     CommandArg::new("token=abc".into()),
/// ];
/// let redacted = redact_sensitive_args(&args);
/// assert_eq!(redacted[1].as_str(), "token=***REDACTED***");
/// ```
#[must_use]
pub fn redact_sensitive_args(args: &[CommandArg]) -> Vec<CommandArg> {
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
pub fn create_temp_ninja_file(content: &NinjaContent) -> AnyResult<NamedTempFile> {
    let tmp = Builder::new()
        .prefix("netsuke.")
        .suffix(".ninja")
        .tempfile()
        .context("create temp file")?;
    write_ninja_file(tmp.path(), content)?;
    Ok(tmp)
}

/// Write `content` to `path` within `dir`.
///
/// # Errors
///
/// Returns an error if the parent directories cannot be created or the file cannot be written.
pub fn write_ninja_file_utf8(
    dir: &cap_fs::Dir,
    path: &Utf8Path,
    content: &NinjaContent,
) -> AnyResult<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_str().is_empty()) {
        dir.create_dir_all(parent.as_str())
            .with_context(|| format!("failed to create parent directory {parent}"))?;
    }
    let mut file = dir
        .create(path.as_str())
        .with_context(|| format!("failed to create Ninja file at {path}"))?;
    file.write_all(content.as_str().as_bytes())
        .with_context(|| format!("failed to write Ninja file to {path}"))?;
    file.flush()
        .with_context(|| format!("failed to flush Ninja file at {path}"))?;
    file.sync_all()
        .with_context(|| format!("failed to sync Ninja file at {path}"))?;
    Ok(())
}

fn derive_dir_and_relative(path: &Utf8Path) -> AnyResult<(cap_fs::Dir, Utf8PathBuf)> {
    if path.is_relative() {
        let dir = cap_fs::Dir::open_ambient_dir(".", ambient_authority())
            .context("open ambient directory")?;
        return Ok((dir, path.to_owned()));
    }

    let mut ancestors = path.ancestors();
    ancestors.next(); // skip the full path
    let (base, dir) = ancestors
        .find_map(|candidate| {
            cap_fs::Dir::open_ambient_dir(candidate.as_str(), ambient_authority())
                .ok()
                .map(|dir| (candidate.to_owned(), dir))
        })
        .ok_or_else(|| anyhow!("no existing ancestor for {path}"))?;
    let relative = path
        .strip_prefix(&base)
        .context("derive relative Ninja path")?
        .to_owned();
    Ok((dir, relative))
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
pub fn write_ninja_file(path: &Path, content: &NinjaContent) -> AnyResult<()> {
    let utf8_path =
        Utf8Path::from_path(path).ok_or_else(|| anyhow!("non-UTF-8 path is not supported"))?;
    let (dir, relative) = derive_dir_and_relative(utf8_path)?;
    write_ninja_file_utf8(&dir, &relative, content)?;
    info!("Generated Ninja file at {utf8_path}");
    Ok(())
}

fn resolve_ninja_program_utf8_with<F>(mut read_env: F) -> Utf8PathBuf
where
    F: FnMut(&str) -> Option<OsString>,
{
    read_env(NINJA_ENV)
        .and_then(|value| {
            let path = PathBuf::from(value);
            Utf8PathBuf::from_path_buf(path).ok()
        })
        .unwrap_or_else(|| Utf8PathBuf::from(NINJA_PROGRAM))
}

#[must_use]
pub fn resolve_ninja_program_utf8() -> Utf8PathBuf {
    resolve_ninja_program_utf8_with(|key| env::var_os(key))
}

/// Determine which Ninja executable to invoke.
#[must_use]
pub fn resolve_ninja_program() -> PathBuf {
    resolve_ninja_program_utf8().into()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_sensitive_keyword_only_flags_known_keys() {
        let token = CommandArg::new(String::from("token=abc"));
        assert!(contains_sensitive_keyword(&token));

        let positional = CommandArg::new(String::from("secrets.yml"));
        assert!(!contains_sensitive_keyword(&positional));

        let path_arg = CommandArg::new(String::from("path=/tmp/secrets.yml"));
        assert!(!contains_sensitive_keyword(&path_arg));

        let spaced = CommandArg::new(String::from("  PASSWORD = value "));
        assert!(contains_sensitive_keyword(&spaced));
    }

    #[test]
    fn redact_argument_preserves_non_sensitive_pairs() {
        let redacted = redact_argument(&CommandArg::new(String::from("auth = token123")));
        assert_eq!(redacted.as_str(), "auth=***REDACTED***");

        let untouched = redact_argument(&CommandArg::new(String::from("path=/var/secrets")));
        assert_eq!(untouched.as_str(), "path=/var/secrets");

        let positional = redact_argument(&CommandArg::new(String::from("secret")));
        assert_eq!(positional.as_str(), "secret");
    }

    #[test]
    fn create_temp_ninja_file_persists_contents() {
        use std::io::Read;

        let content = NinjaContent::new(String::from("rule cc"));
        let file = create_temp_ninja_file(&content).expect("create temp file");
        let mut reopened = file.reopen().expect("reopen temp file");
        let mut written = String::new();
        reopened
            .read_to_string(&mut written)
            .expect("read temp file");
        assert_eq!(written, content.as_str());
        assert!(file.path().to_string_lossy().ends_with(".ninja"));
    }

    #[test]
    fn write_ninja_file_utf8_creates_parent_directories() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let dir =
            cap_fs::Dir::open_ambient_dir(temp.path(), ambient_authority()).expect("open temp dir");
        let nested = Utf8PathBuf::from("nested/build.ninja");
        let content = NinjaContent::new(String::from("build all: phony"));

        write_ninja_file_utf8(&dir, &nested, &content).expect("write ninja file");

        let nested_path = temp.path().join("nested").join("build.ninja");
        let written = std::fs::read_to_string(&nested_path).expect("read nested file");
        assert_eq!(written, content.as_str());
        assert!(nested_path.parent().expect("parent path").exists());
    }

    #[test]
    fn write_ninja_file_handles_absolute_paths() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let nested = temp.path().join("nested").join("build.ninja");
        let content = NinjaContent::new(String::from("build all: phony"));

        write_ninja_file(&nested, &content).expect("write ninja file");

        let written = std::fs::read_to_string(&nested).expect("read nested file");
        assert_eq!(written, content.as_str());
        assert!(nested.parent().expect("parent path").exists());
    }

    #[test]
    fn resolve_ninja_program_utf8_prefers_env_override() {
        let resolved = resolve_ninja_program_utf8_with(|_| Some(OsString::from("/opt/ninja")));
        assert_eq!(resolved, Utf8PathBuf::from("/opt/ninja"));
    }

    #[test]
    fn resolve_ninja_program_utf8_defaults_without_override() {
        let resolved = resolve_ninja_program_utf8_with(|_| None);
        assert_eq!(resolved, Utf8PathBuf::from(NINJA_PROGRAM));
    }

    #[cfg(unix)]
    #[test]
    fn resolve_ninja_program_utf8_ignores_invalid_utf8_override() {
        use std::os::unix::ffi::OsStringExt;

        let resolved = resolve_ninja_program_utf8_with(|_| {
            Some(OsString::from_vec(vec![0xff, b'n', b'i', b'n', b'j', b'a']))
        });
        assert_eq!(resolved, Utf8PathBuf::from(NINJA_PROGRAM));
    }
}
