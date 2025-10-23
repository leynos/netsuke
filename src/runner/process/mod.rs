//! Process helpers for Ninja file lifecycle, argument redaction, and subprocess I/O.
//! Internal to `runner`; public API is defined in `runner.rs`.

use super::{BuildTargets, NINJA_PROGRAM};
use crate::cli::Cli;
use camino::Utf8PathBuf;
use ninja_env::NINJA_ENV;
use std::{
    env,
    ffi::OsString,
    io::{self, BufReader, ErrorKind, Read, Write},
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
// Re-export redaction helpers only for doctests so the examples compile without
// introducing unused imports in normal builds.
#[cfg(doctest)]
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
    let Some(stdout) = child.stdout.take() else {
        terminate_child(&mut child, "stdout pipe unavailable");
        return Err(io::Error::other("child process missing stdout pipe"));
    };
    let Some(stderr) = child.stderr.take() else {
        terminate_child(&mut child, "stderr pipe unavailable");
        return Err(io::Error::other("child process missing stderr pipe"));
    };

    let out_handle =
        thread::spawn(move || forward_child_output(BufReader::new(stdout), io::stdout(), "stdout"));
    let err_handle =
        thread::spawn(move || forward_child_output(BufReader::new(stderr), io::stderr(), "stderr"));

    let status = child.wait()?;
    match out_handle.join() {
        Ok(stdout_stats) => {
            if stdout_stats.write_failed {
                tracing::debug!("stdout forwarding encountered closed pipe; output truncated");
            }
        }
        Err(err) => {
            tracing::warn!("stdout forwarding thread panicked: {err:?}");
        }
    }
    match err_handle.join() {
        Ok(stderr_stats) => {
            if stderr_stats.write_failed {
                tracing::debug!("stderr forwarding encountered closed pipe; output truncated");
            }
        }
        Err(err) => {
            tracing::warn!("stderr forwarding thread panicked: {err:?}");
        }
    }
    Ok(status)
}

fn terminate_child(child: &mut Child, context: &str) {
    if let Err(err) = child.kill() {
        tracing::debug!("failed to kill child after {context}: {err}");
    }
    if let Err(err) = child.wait() {
        tracing::debug!("failed to reap child after {context}: {err}");
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ForwardStats {
    bytes_read: usize,
    bytes_written: usize,
    write_failed: bool,
}

struct CountingWriter<'a, W> {
    inner: &'a mut W,
    written: u64,
}

impl<W: Write> Write for CountingWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.inner.write(buf) {
            Ok(count) => {
                self.written += count as u64;
                Ok(count)
            }
            Err(err) => Err(err),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn clamp_u64_to_usize(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn forward_child_output<R, W>(
    mut reader: R,
    mut writer: W,
    stream_name: &'static str,
) -> ForwardStats
where
    R: Read,
    W: Write,
{
    let mut stats = ForwardStats::default();
    let mut counting_writer = CountingWriter {
        inner: &mut writer,
        written: 0,
    };

    match io::copy(&mut reader, &mut counting_writer) {
        Ok(bytes) => {
            stats.bytes_written = clamp_u64_to_usize(counting_writer.written);
            stats.bytes_read = clamp_u64_to_usize(bytes);
        }
        Err(err) => {
            stats.write_failed = true;
            stats.bytes_written = clamp_u64_to_usize(counting_writer.written);
            stats.bytes_read = clamp_u64_to_usize(counting_writer.written);
            tracing::debug!(
                "Failed to write child {stream_name} output to parent: {err}; discarding remaining bytes"
            );
            match io::copy(&mut reader, &mut io::sink()) {
                Ok(drained) => {
                    let drained_bytes = clamp_u64_to_usize(drained);
                    stats.bytes_read = stats.bytes_read.saturating_add(drained_bytes);
                }
                Err(drain_err) => {
                    tracing::debug!(
                        "Failed to drain child {stream_name} output after writer closed: {drain_err}"
                    );
                }
            }
        }
    }

    stats
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
    use std::{
        ffi::OsString,
        io::Cursor,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    #[derive(Clone)]
    struct FailingWriter {
        writes: Arc<AtomicUsize>,
    }

    impl FailingWriter {
        fn new(writes: Arc<AtomicUsize>) -> Self {
            Self { writes }
        }
    }

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            let previous = self.writes.fetch_add(1, Ordering::SeqCst);
            let error_kind = if previous == 0 {
                io::ErrorKind::BrokenPipe
            } else {
                io::ErrorKind::Other
            };
            Err(io::Error::new(error_kind, "sink closed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
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

    #[test]
    fn forward_output_writes_all_bytes_when_parent_alive() {
        let input = b"alpha\nbravo\ncharlie\n".to_vec();
        let reader = BufReader::new(Cursor::new(input.clone()));
        let stats = forward_child_output(reader, Vec::new(), "stdout");

        assert_eq!(stats.bytes_read, input.len());
        assert_eq!(stats.bytes_written, input.len());
        assert!(!stats.write_failed);
    }

    #[test]
    fn forward_output_continues_draining_after_write_failure() {
        let input = b"echo-one\necho-two\necho-three\n".to_vec();
        let reader = BufReader::new(Cursor::new(input.clone()));
        let write_attempts = Arc::new(AtomicUsize::new(0));
        let failing_writer = FailingWriter::new(write_attempts.clone());
        let stats = forward_child_output(reader, failing_writer, "stdout");

        assert_eq!(stats.bytes_read, input.len());
        assert_eq!(write_attempts.load(Ordering::SeqCst), 1);
        assert!(stats.write_failed);
        assert_eq!(stats.bytes_written, 0);
    }
}
