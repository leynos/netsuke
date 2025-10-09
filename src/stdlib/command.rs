//! Shell-oriented helpers for the `MiniJinja` standard library.
//!
//! The helpers bridge template values into the local shell while keeping
//! behaviour predictable across platforms. All helpers mark the stdlib state as
//! impure so the caller can invalidate any caching layer that depends on pure
//! template evaluation.

use std::{
    io::{self, Write},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use minijinja::{
    Error, ErrorKind, State,
    value::{Value, ValueKind},
};
use shell_quote::{QuoteRefExt, Sh};

#[cfg(windows)]
const SHELL: &str = "cmd";
#[cfg(windows)]
const SHELL_ARGS: &[&str] = &["/C"];

#[cfg(not(windows))]
const SHELL: &str = "sh";
#[cfg(not(windows))]
const SHELL_ARGS: &[&str] = &["-c"];

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

fn quote(arg: &str) -> String {
    let bytes = arg.quoted(Sh);
    String::from_utf8(bytes).expect("quoted args are valid UTF-8")
}

fn to_bytes(value: &Value) -> Result<Vec<u8>, Error> {
    if value.is_undefined() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "shell filter cannot act on undefined values",
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

    let mut child = cmd.spawn().map_err(CommandFailure::Spawn)?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input).map_err(CommandFailure::Io)?;
    }
    let output = child.wait_with_output().map_err(CommandFailure::Io)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(CommandFailure::Exit {
            status: output.status.code(),
            stderr: output.stderr,
        })
    }
}

fn value_from_bytes(bytes: Vec<u8>) -> Value {
    String::from_utf8(bytes.clone()).map_or_else(|_| Value::from_bytes(bytes), Value::from)
}

fn command_error(err: CommandFailure, template: &str, command: &str) -> Error {
    match err {
        CommandFailure::Spawn(spawn) => Error::new(
            ErrorKind::InvalidOperation,
            format!("failed to spawn shell for '{command}' in template '{template}': {spawn}"),
        ),
        CommandFailure::Io(io_err) => Error::new(
            ErrorKind::InvalidOperation,
            format!("shell command '{command}' in template '{template}' failed: {io_err}"),
        ),
        CommandFailure::Exit { status, stderr } => {
            let mut msg = status.map_or_else(
                || {
                    format!(
                        "shell command '{command}' in template '{template}' terminated by signal"
                    )
                },
                |code| {
                    format!(
                        "shell command '{command}' in template '{template}' exited with status {code}"
                    )
                },
            );
            let stderr = String::from_utf8_lossy(&stderr);
            let trimmed = stderr.trim();
            if !trimmed.is_empty() {
                msg.push_str(": ");
                msg.push_str(trimmed);
            }
            Error::new(ErrorKind::InvalidOperation, msg)
        }
    }
}

enum CommandFailure {
    Spawn(io::Error),
    Io(io::Error),
    Exit {
        status: Option<i32>,
        stderr: Vec<u8>,
    },
}

impl From<io::Error> for CommandFailure {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}
