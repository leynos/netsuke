//! Shell-oriented helpers for the `MiniJinja` standard library.
//!
//! The helpers bridge template values into the local shell while keeping
//! behaviour predictable across platforms. All helpers mark the stdlib state as
//! impure so the caller can invalidate any caching layer that depends on pure
//! template evaluation.
//!
//! # Security
//!
//! The `shell` and `grep` filters execute external commands based on template
//! content. Templates using these filters must come from trusted sources only.
//! Never allow untrusted input to control command strings or patterns, as this
//! enables arbitrary code execution.

use std::{
    fmt::Write as FmtWrite,
    io::{self, Read, Write},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use super::value_from_bytes;
use minijinja::{
    Error, ErrorKind, State,
    value::{Value, ValueKind},
};
#[cfg(not(windows))]
use shell_quote::{QuoteRefExt, Sh};
use wait_timeout::ChildExt;

#[cfg(windows)]
const SHELL: &str = "cmd";
#[cfg(windows)]
const SHELL_ARGS: &[&str] = &["/C"];

#[cfg(not(windows))]
const SHELL: &str = "sh";
#[cfg(not(windows))]
const SHELL_ARGS: &[&str] = &["-c"];

// Cap commands at five seconds so template renders fail fast on hung helpers
// while still allowing short, legitimate processes to complete. The limit keeps
// feedback responsive during behavioural tests without imposing noticeable
// delays for happy-path renders.
// Commands run during template evaluation must not hang renders. Five seconds
// keeps `shell` and `grep` responsive for tests and typical helper binaries
// while still surfacing timeouts for misbehaving commands.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

/// Registers shell-oriented filters in the `MiniJinja` environment.
///
/// The `shell` filter executes arbitrary shell commands and returns their
/// stdout. The `grep` filter searches input text using the `grep` utility.
/// Both filters mark the provided `impure` flag to signal that template
/// evaluation has side effects and should not be cached.
///
/// # Security
///
/// Only use these filters with trusted templates. See the module-level
/// documentation for further details about the associated risks.
pub(crate) fn register(env: &mut minijinja::Environment<'_>, impure: Arc<AtomicBool>) {
    let shell_flag = Arc::clone(&impure);
    env.add_filter(
        "shell",
        move |state: &State, value: Value, command: String| {
            shell_flag.store(true, Ordering::Relaxed);
            execute_shell(state, &value, &command)
        },
    );

    let grep_flag = impure;
    env.add_filter(
        "grep",
        move |state: &State, value: Value, pattern: String, flags: Option<Value>| {
            grep_flag.store(true, Ordering::Relaxed);
            execute_grep(state, &value, &pattern, flags)
        },
    );
}

fn execute_shell(state: &State, value: &Value, command: &str) -> Result<Value, Error> {
    let cmd = command.trim();
    if cmd.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "shell filter requires a non-empty command",
        ));
    }

    let input = to_bytes(value)?;
    let output = run_command(cmd, &input).map_err(|err| command_error(err, state.name(), cmd))?;
    Ok(value_from_bytes(output))
}

fn execute_grep(
    state: &State,
    value: &Value,
    pattern: &str,
    flags: Option<Value>,
) -> Result<Value, Error> {
    if pattern.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "grep filter requires a search pattern",
        ));
    }

    let mut args = collect_flag_args(flags)?;
    args.push(pattern.to_owned());
    let command = format_command("grep", &args);
    let input = to_bytes(value)?;

    #[cfg(windows)]
    let output = run_program("grep", &args, &input)
        .map_err(|err| command_error(err, state.name(), &command))?;

    #[cfg(not(windows))]
    let output =
        run_command(&command, &input).map_err(|err| command_error(err, state.name(), &command))?;

    Ok(value_from_bytes(output))
}

fn collect_flag_args(flags: Option<Value>) -> Result<Vec<String>, Error> {
    let Some(value) = flags else {
        return Ok(Vec::new());
    };
    match value.kind() {
        ValueKind::Undefined => Ok(Vec::new()),
        ValueKind::Seq | ValueKind::Iterable => value
            .try_iter()?
            .map(|item| {
                item.as_str().map_or_else(
                    || {
                        Err(Error::new(
                            ErrorKind::InvalidOperation,
                            "grep flags must be strings",
                        ))
                    },
                    |s| Ok(s.to_owned()),
                )
            })
            .collect(),
        _ => value
            .as_str()
            .map(|s| vec![s.to_owned()])
            .ok_or_else(|| Error::new(ErrorKind::InvalidOperation, "grep flags must be strings")),
    }
}

fn format_command(base: &str, args: &[String]) -> String {
    let mut command = String::from(base);
    for arg in args {
        command.push(' ');
        command.push_str(&quote(arg));
    }
    command
}

#[cfg(windows)]
fn quote(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".to_owned();
    }
    let requires_quotes = arg.chars().any(|ch| ch.is_whitespace() || ch == '"');
    if !requires_quotes {
        return arg.to_owned();
    }

    let mut buf = String::with_capacity(arg.len() + 2);
    buf.push('"');
    // Double quotation marks so readers understand how cmd.exe interprets them.
    for ch in arg.chars() {
        if ch == '"' {
            buf.push('"');
        }
        buf.push(ch);
    }
    buf.push('"');
    buf
}

#[cfg(not(windows))]
fn quote(arg: &str) -> String {
    let bytes = arg.quoted(Sh);
    String::from_utf8(bytes).expect("quoted args are valid UTF-8")
}

fn to_bytes(value: &Value) -> Result<Vec<u8>, Error> {
    if value.is_undefined() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "input value is undefined",
        ));
    }

    if let Some(bytes) = value.as_bytes() {
        return Ok(bytes.to_vec());
    }

    Ok(value.to_string().into_bytes())
}

fn run_command(command: &str, input: &[u8]) -> Result<Vec<u8>, CommandFailure> {
    let mut cmd = Command::new(SHELL);
    cmd.args(SHELL_ARGS)
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    run_child(cmd, input)
}

#[cfg(windows)]
fn run_program(program: &str, args: &[String], input: &[u8]) -> Result<Vec<u8>, CommandFailure> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    run_child(cmd, input)
}

fn run_child(mut command: Command, input: &[u8]) -> Result<Vec<u8>, CommandFailure> {
    let mut child = command.spawn().map_err(CommandFailure::Spawn)?;
    let mut stdin_handle = child.stdin.take().map(|mut stdin| {
        let buffer = input.to_vec();
        thread::spawn(move || stdin.write_all(&buffer))
    });

    let mut stdout_reader = spawn_pipe_reader(child.stdout.take());
    let mut stderr_reader = spawn_pipe_reader(child.stderr.take());

    let status = match wait_for_exit(&mut child, COMMAND_TIMEOUT) {
        Ok(status) => status,
        Err(err) => {
            cleanup_readers(&mut stdout_reader, &mut stderr_reader, &mut stdin_handle);
            return Err(err);
        }
    };

    let stdout = join_reader(stdout_reader.take()).map_err(CommandFailure::Io)?;
    let stderr = join_reader(stderr_reader.take()).map_err(CommandFailure::Io)?;

    handle_stdin_result(stdin_handle.take(), status.code(), &stderr)?;

    if status.success() {
        Ok(stdout)
    } else {
        Err(CommandFailure::Exit {
            status: status.code(),
            stderr,
        })
    }
}

fn cleanup_readers(
    stdout_reader: &mut Option<thread::JoinHandle<io::Result<Vec<u8>>>>,
    stderr_reader: &mut Option<thread::JoinHandle<io::Result<Vec<u8>>>>,
    stdin_handle: &mut Option<thread::JoinHandle<io::Result<()>>>,
) {
    let _ = join_reader(stdout_reader.take());
    let _ = join_reader(stderr_reader.take());
    if let Some(handle) = stdin_handle.take() {
        let _ = handle.join();
    }
}

fn handle_stdin_result(
    stdin_handle: Option<thread::JoinHandle<io::Result<()>>>,
    status: Option<i32>,
    stderr: &[u8],
) -> Result<(), CommandFailure> {
    let Some(handle) = stdin_handle else {
        return Ok(());
    };

    match handle.join() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => {
            if err.kind() == io::ErrorKind::BrokenPipe {
                if status == Some(0) {
                    return Ok(());
                }
                return Err(CommandFailure::BrokenPipe {
                    source: err,
                    status,
                    stderr: stderr.to_vec(),
                });
            }
            Err(CommandFailure::Io(err))
        }
        Err(_) => Err(CommandFailure::Io(io::Error::other(
            "stdin writer panicked",
        ))),
    }
}

fn command_error(err: CommandFailure, template: &str, command: &str) -> Error {
    match err {
        CommandFailure::Spawn(spawn) => Error::new(
            ErrorKind::InvalidOperation,
            format!("failed to spawn command '{command}' in template '{template}': {spawn}"),
        ),
        CommandFailure::Io(io_err) => {
            let pipe_msg = if io_err.kind() == io::ErrorKind::BrokenPipe {
                " (command closed input early)"
            } else {
                ""
            };
            Error::new(
                ErrorKind::InvalidOperation,
                format!("command '{command}' in template '{template}' failed: {io_err}{pipe_msg}"),
            )
        }
        CommandFailure::BrokenPipe {
            source,
            status,
            stderr,
        } => {
            let mut msg = format!(
                "command '{command}' in template '{template}' failed: {source} (command closed input early)"
            );
            if let Some(code) = status {
                let _ = FmtWrite::write_fmt(&mut msg, format_args!("; exited with status {code}"));
            } else {
                msg.push_str("; terminated by signal");
            }
            append_stderr(&mut msg, &stderr);
            Error::new(ErrorKind::InvalidOperation, msg)
        }
        CommandFailure::Exit { status, stderr } => {
            let mut msg = status.map_or_else(
                || format!("command '{command}' in template '{template}' terminated by signal"),
                |code| {
                    format!(
                        "command '{command}' in template '{template}' exited with status {code}"
                    )
                },
            );
            append_stderr(&mut msg, &stderr);
            Error::new(ErrorKind::InvalidOperation, msg)
        }
        CommandFailure::Timeout(duration) => Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "command '{command}' in template '{template}' timed out after {}s",
                duration.as_secs()
            ),
        ),
    }
}

enum CommandFailure {
    Spawn(io::Error),
    Io(io::Error),
    BrokenPipe {
        source: io::Error,
        status: Option<i32>,
        stderr: Vec<u8>,
    },
    Exit {
        status: Option<i32>,
        stderr: Vec<u8>,
    },
    Timeout(Duration),
}

impl From<io::Error> for CommandFailure {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> Result<ExitStatus, CommandFailure> {
    if let Some(status) = child.wait_timeout(timeout).map_err(CommandFailure::Io)? {
        Ok(status)
    } else {
        if let Err(err) = child.kill()
            && err.kind() != io::ErrorKind::InvalidInput
        {
            return Err(CommandFailure::Io(err));
        }
        let _ = child.wait();
        Err(CommandFailure::Timeout(timeout))
    }
}

fn spawn_pipe_reader<R>(pipe: Option<R>) -> Option<thread::JoinHandle<io::Result<Vec<u8>>>>
where
    R: Read + Send + 'static,
{
    pipe.map(|mut reader| {
        thread::spawn(move || {
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf)?;
            Ok(buf)
        })
    })
}

fn join_reader(handle: Option<thread::JoinHandle<io::Result<Vec<u8>>>>) -> io::Result<Vec<u8>> {
    handle.map_or_else(
        || Ok(Vec::new()),
        |handle| {
            handle
                .join()
                .map_err(|_| io::Error::other("pipe reader panicked"))?
        },
    )
}

fn append_stderr(message: &mut String, stderr: &[u8]) {
    let stderr = String::from_utf8_lossy(stderr);
    let trimmed = stderr.trim();
    if !trimmed.is_empty() {
        message.push_str(": ");
        message.push_str(trimmed);
    }
}
