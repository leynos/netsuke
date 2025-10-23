//! Process helpers for Ninja file lifecycle, argument redaction, and subprocess I/O.
//! Internal to `runner`; public API is defined in `runner.rs`.

use super::{BuildTargets, NINJA_PROGRAM};
use crate::cli::Cli;
use camino::Utf8PathBuf;
use ninja_env::NINJA_ENV;
use std::{
    env,
    ffi::OsString,
    io::{self, BufRead, BufReader, ErrorKind, Write},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
};
use tracing::info;

mod file_io;
mod paths;
mod redaction;

pub use file_io::*;
pub use paths::*;
// Re-export redaction helpers for doctests without leaking unused imports in release builds.
#[cfg_attr(
    not(doctest),
    expect(unused_imports, reason = "retain doctest re-exports")
)]
pub use redaction::*;

use redaction::{CommandArg, redact_sensitive_args};

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

#[must_use]
pub fn resolve_ninja_program() -> PathBuf {
    resolve_ninja_program_utf8().into()
}

fn configure_ninja_command(
    cmd: &mut Command,
    cli: &Cli,
    build_file: &Path,
    targets: &BuildTargets<'_>,
) -> io::Result<()> {
    if let Some(dir) = &cli.directory {
        let canonical = canonicalize_utf8_path(dir.as_path())?;
        cmd.current_dir(canonical.as_std_path());
    }
    if let Some(jobs) = cli.jobs {
        cmd.arg("-j").arg(jobs.to_string());
    }
    let build_file_path = canonicalize_utf8_path(build_file).or_else(|_| {
        Utf8PathBuf::from_path_buf(build_file.to_path_buf()).map_err(|_| {
            io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "build file path {} is not valid UTF-8",
                    build_file.display()
                ),
            )
        })
    })?;
    cmd.arg("-f").arg(build_file_path.as_std_path());
    cmd.args(targets.as_slice());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    Ok(())
}

fn log_command_execution(cmd: &Command) {
    let program_path = PathBuf::from(cmd.get_program());
    let program_display = Utf8PathBuf::from_path_buf(program_path.clone()).map_or_else(
        |_| program_path.to_string_lossy().into_owned(),
        Utf8PathBuf::into_string,
    );
    let args: Vec<CommandArg> = cmd
        .get_args()
        .map(|a| CommandArg::new(a.to_string_lossy().into_owned()))
        .collect();
    let redacted_args = redact_sensitive_args(&args);
    let arg_strings: Vec<&str> = redacted_args.iter().map(CommandArg::as_str).collect();
    info!(
        "Running command: {} {}",
        program_display,
        arg_strings.join(" ")
    );
}

/// Invoke the Ninja executable with the provided CLI settings.
///
/// The function forwards the job count and working directory to Ninja,
/// specifies the temporary build file, and streams its standard output and
/// error back to the user.
///
/// # Errors
///
/// Returns an [`io::Error`] if the Ninja process fails to spawn, the standard
/// streams are unavailable, or when Ninja reports a non-zero exit status.
///
pub fn run_ninja(
    program: &Path,
    cli: &Cli,
    build_file: &Path,
    targets: &BuildTargets<'_>,
) -> io::Result<()> {
    let mut cmd = Command::new(program);
    configure_ninja_command(&mut cmd, cli, build_file, targets)?;
    log_command_execution(&cmd);
    let child = cmd.spawn()?;
    let status = spawn_and_stream_output(child)?;
    check_exit_status(status)
}

fn spawn_and_stream_output(mut child: Child) -> io::Result<ExitStatus> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("child process missing stdout pipe"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("child process missing stderr pipe"))?;

    let out_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut handle = io::stdout();
        for line in reader.lines().map_while(Result::ok) {
            if writeln!(handle, "{line}").is_err() {
                break;
            }
        }
    });
    let err_handle = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut handle = io::stderr();
        for line in reader.lines().map_while(Result::ok) {
            if writeln!(handle, "{line}").is_err() {
                break;
            }
        }
    });

    let status = child.wait()?;
    if let Err(err) = out_handle.join() {
        tracing::warn!("stdout forwarding thread panicked: {err:?}");
    }
    if let Err(err) = err_handle.join() {
        tracing::warn!("stderr forwarding thread panicked: {err:?}");
    }
    Ok(status)
}

fn check_exit_status(status: ExitStatus) -> io::Result<()> {
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
    use camino::Utf8PathBuf;
    use std::ffi::OsString;

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
