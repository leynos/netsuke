//! Resolve, probe, and bind the Windows Bash executable selected by `PATH`.
//!
//! This runner-owned adapter searches only the injected `PATH` entries. It
//! deliberately has no application-directory, system-directory, or workspace
//! fallback, so preflight and generated commands agree on executable identity.

use std::{
    ffi::OsString,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
};

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use mockable::Env;

use crate::{recipe_shell::BashExecutable, recipe_shell::RecipeShell};

use super::telemetry::{self, BashProbeOutcome};

/// Bound the amount of child output retained for a failed Bash preflight.
const MAX_PROBE_OUTPUT_BYTES: usize = 2048;

/// Record the result of running the Bash capability probe.
#[derive(Debug, Eq, PartialEq)]
enum BashProbeStatus {
    /// Confirm that the selected executable accepted `--version`.
    Available,
    /// Preserve the selected executable's failed status and bounded output.
    Failed {
        /// Retain the process exit status for actionable diagnostics.
        status: String,
        /// Retain the bounded, sanitised process output.
        output: String,
    },
}

/// Supply platform, shell and filesystem inputs for one Bash preflight.
struct BashRuntimeContext<'a> {
    /// Select whether Windows shell lookup applies to this operation.
    is_windows: bool,
    /// Select the recipe shell whose runtime may need resolution.
    shell: RecipeShell,
    /// Read the environment snapshot used to locate Bash.
    env: &'a dyn Env,
    /// Resolve relative PATH entries against the operation's current directory.
    current_dir: &'a Path,
}

/// Resolve and validate Bash only for a Windows build that selected Bash.
///
/// # Errors
///
/// Returns an error when the current directory, injected `PATH`, or selected
/// executable cannot be resolved, or the selected Bash fails `--version`.
pub(in crate::runner) fn resolve_bash_runtime(
    shell: RecipeShell,
) -> Result<Option<BashExecutable>> {
    if !cfg!(windows) || shell != RecipeShell::Bash {
        return Ok(None);
    }
    let current_dir =
        std::env::current_dir().context("read the current directory for Bash lookup")?;
    resolve_bash_runtime_with(
        &BashRuntimeContext {
            is_windows: true,
            shell,
            env: &mockable::DefaultEnv,
            current_dir: &current_dir,
        },
        probe_bash_runtime,
    )
}

/// Resolve one executable from the injected `PATH`, then probe that same path.
///
/// # Errors
///
/// Returns an error when no executable can be selected or the selected
/// executable cannot pass the injected probe.
fn resolve_bash_runtime_with(
    context: &BashRuntimeContext<'_>,
    probe: impl FnOnce(&BashExecutable) -> io::Result<BashProbeStatus>,
) -> Result<Option<BashExecutable>> {
    if !context.is_windows || context.shell != RecipeShell::Bash {
        return Ok(None);
    }

    let (outcome, result) =
        match resolve_bash_from_path(context.env.os_string("PATH"), context.current_dir) {
            Ok(executable) => {
                let probe_result = probe(&executable);
                let outcome = probe_outcome(&probe_result);
                (
                    outcome,
                    validate_bash_probe(&executable, probe_result).map(|()| Some(executable)),
                )
            }
            Err(error) => {
                let outcome = if error
                    .root_cause()
                    .downcast_ref::<io::Error>()
                    .is_some_and(|io_error| io_error.kind() == io::ErrorKind::NotFound)
                {
                    BashProbeOutcome::NotFound
                } else {
                    BashProbeOutcome::LaunchFailed
                };
                (outcome, Err(error))
            }
        };

    telemetry::instrument_bash_preflight(outcome, || result)
}

/// Select the first `bash.exe` file in the supplied `PATH` snapshot.
///
/// # Errors
///
/// Returns an error when `PATH` has no regular `bash.exe` file or an entry
/// cannot be inspected or represented as UTF-8.
fn resolve_bash_from_path(
    path_value: Option<OsString>,
    current_dir: &Path,
) -> Result<BashExecutable> {
    let Some(path_entries) = path_value else {
        return Err(io::Error::new(io::ErrorKind::NotFound, "PATH is not set"))
            .context(missing_bash_guidance());
    };

    let mut first_inspection_error = None;
    for (index, component) in std::env::split_paths(&path_entries).enumerate() {
        if component.as_os_str().is_empty() {
            continue;
        }
        let absolute_directory = absolute_path_entry(component, current_dir);
        let utf8_directory = Utf8PathBuf::from_path_buf(absolute_directory)
            .map_err(|_| anyhow::anyhow!("PATH entry {index} is not a valid UTF-8 path"))?;
        match bash_candidate_in(&utf8_directory) {
            Ok(Some(candidate)) => {
                return BashExecutable::from_absolute_path(candidate)
                    .context("resolved Bash executable must be absolute");
            }
            Ok(None) => {}
            Err(error) if is_missing_path_error(&error) => {}
            Err(error) => {
                if first_inspection_error.is_none() {
                    first_inspection_error = Some(
                        anyhow::Error::new(error)
                            .context(format!("inspect PATH entry {index} for bash.exe")),
                    );
                }
            }
        }
    }

    if let Some(error) = first_inspection_error {
        return Err(error).context("could not inspect PATH entries for Windows Bash");
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no bash.exe file exists in the supplied PATH entries",
    ))
    .context(missing_bash_guidance())
}

/// Return the fixed user guidance shared by Bash lookup and probe failures.
const fn missing_bash_guidance() -> &'static str {
    "Windows legacy recipes selected Bash, but PATH contains no bash.exe; install Git for Windows or MSYS2, add its Bash directory to PATH, or unset NETSUKE_WINDOWS_SHELL to use PowerShell"
}

/// Make a non-empty relative `PATH` entry absolute against the captured cwd.
fn absolute_path_entry(component: PathBuf, current_dir: &Path) -> PathBuf {
    if component.is_absolute() {
        component
    } else {
        current_dir.join(component)
    }
}

/// Return a regular `bash.exe` file directly inside one `PATH` directory.
///
/// # Errors
///
/// Returns an I/O error when the directory or candidate cannot be inspected.
fn bash_candidate_in(directory: &camino::Utf8Path) -> io::Result<Option<Utf8PathBuf>> {
    let handle = Dir::open_ambient_dir(directory, ambient_authority())?;
    let metadata = match handle.metadata("bash.exe") {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(metadata.is_file().then(|| directory.join("bash.exe")))
}

/// Return whether a path lookup error means the candidate is absent or unusable.
fn is_missing_path_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

/// Run `--version` on the exact executable selected from `PATH`.
///
/// # Errors
///
/// Returns an I/O error when the process cannot start, its output cannot be
/// drained, or its exit status cannot be collected.
fn probe_bash_runtime(executable: &BashExecutable) -> io::Result<BashProbeStatus> {
    let mut child = Command::new(executable.as_path().as_str())
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout_pipe = child.stdout.take().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::BrokenPipe,
            "Bash probe stdout was not captured",
        )
    })?;
    let stderr_pipe = child.stderr.take().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::BrokenPipe,
            "Bash probe stderr was not captured",
        )
    })?;
    let (status, stdout_bytes, stderr_bytes) = thread::scope(|scope| {
        let stdout_reader = scope.spawn(|| read_bounded_and_drain(stdout_pipe));
        let stderr_reader = scope.spawn(|| read_bounded_and_drain(stderr_pipe));
        let status = child.wait()?;
        let captured_stdout = join_reader(stdout_reader)?;
        let captured_stderr = join_reader(stderr_reader)?;
        Ok::<_, io::Error>((status, captured_stdout, captured_stderr))
    })?;
    let output = describe_probe_output(&stdout_bytes, &stderr_bytes);
    Ok(if status.success() {
        BashProbeStatus::Available
    } else {
        BashProbeStatus::Failed {
            status: status.to_string(),
            output,
        }
    })
}

/// Read at most the configured limit while draining the pipe to prevent blockage.
///
/// # Errors
///
/// Returns an I/O error when reading the child output pipe fails.
fn read_bounded_and_drain(mut reader: impl Read) -> io::Result<Vec<u8>> {
    let mut captured = Vec::with_capacity(MAX_PROBE_OUTPUT_BYTES);
    let mut buffer = [0u8; 512];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(captured);
        }
        let remaining = MAX_PROBE_OUTPUT_BYTES.saturating_sub(captured.len());
        let captured_length = read.min(remaining);
        captured.extend(buffer.iter().take(captured_length).copied());
    }
}

/// Join one output reader and preserve reader errors without panicking.
///
/// # Errors
///
/// Returns the reader's I/O error or a join error if its thread panicked.
fn join_reader(reader: thread::ScopedJoinHandle<'_, io::Result<Vec<u8>>>) -> io::Result<Vec<u8>> {
    reader
        .join()
        .map_err(|_| io::Error::other("Bash probe output reader panicked"))?
}

/// Render bounded probe output while replacing control characters.
fn describe_probe_output(stdout_bytes: &[u8], stderr_bytes: &[u8]) -> String {
    let stdout_text = sanitized_output(stdout_bytes);
    let stderr_text = sanitized_output(stderr_bytes);
    match (stdout_text.is_empty(), stderr_text.is_empty()) {
        (true, true) => "no output".into(),
        (false, true) => format!("stdout: {stdout_text}"),
        (true, false) => format!("stderr: {stderr_text}"),
        (false, false) => format!("stdout: {stdout_text}; stderr: {stderr_text}"),
    }
}

/// Convert one bounded child-output fragment into single-line diagnostic text.
fn sanitized_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Convert one probe result into a bounded telemetry category.
fn probe_outcome(result: &io::Result<BashProbeStatus>) -> BashProbeOutcome {
    match result {
        Ok(BashProbeStatus::Available) => BashProbeOutcome::Success,
        Ok(BashProbeStatus::Failed { .. }) => BashProbeOutcome::NonZeroExit,
        Err(error) if error.kind() == io::ErrorKind::NotFound => BashProbeOutcome::NotFound,
        Err(_) => BashProbeOutcome::LaunchFailed,
    }
}

/// Validate the status from a probe of the selected executable.
///
/// # Errors
///
/// Returns an error when the process could not launch or exited unsuccessfully.
fn validate_bash_probe(
    executable: &BashExecutable,
    result: io::Result<BashProbeStatus>,
) -> Result<()> {
    let probe_status = result.map_err(|error| {
        let message = if error.kind() == io::ErrorKind::NotFound {
            format!(
                "selected Bash executable `{}` disappeared before `--version` could start; \
                 install Git for Windows or MSYS2, repair PATH, or unset NETSUKE_WINDOWS_SHELL",
                executable.as_path()
            )
        } else {
            format!(
                "could not start selected Bash executable `{}` with `--version`; \
                 repair the runtime or unset NETSUKE_WINDOWS_SHELL to use PowerShell",
                executable.as_path()
            )
        };
        anyhow::Error::new(error).context(message)
    })?;
    if let BashProbeStatus::Failed { status, output } = probe_status {
        anyhow::bail!(
            "selected Bash executable `{}` with `--version` exited with {status}; {output}; \
             repair the runtime or unset NETSUKE_WINDOWS_SHELL to use PowerShell",
            executable.as_path()
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
