//! Filter implementations exposed to `MiniJinja` templates.

use crate::localization::{self, keys};
use minijinja::{
    Error, ErrorKind, State,
    value::{Value, ValueKind},
};

#[cfg(windows)]
use super::execution::run_program;
use super::{
    context::{CommandContext, GrepCall},
    error::command_error,
    execution::run_command,
    quote::quote,
    result::StdoutResult,
    value_from_bytes,
};

pub(super) fn execute_shell(
    state: &State,
    value: &Value,
    command: &str,
    context: &CommandContext,
) -> Result<Value, Error> {
    let cmd = command.trim();
    if cmd.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::COMMAND_SHELL_EMPTY).to_string(),
        ));
    }

    let input = to_bytes(value)?;
    let output =
        run_command(cmd, &input, context).map_err(|err| command_error(err, state.name(), cmd))?;
    match output {
        StdoutResult::Bytes(bytes) => Ok(value_from_bytes(bytes)),
        StdoutResult::Tempfile(path) => Ok(Value::from(path.as_str())),
    }
}

pub(super) fn execute_grep(
    state: &State,
    value: &Value,
    call: GrepCall<'_>,
    context: &CommandContext,
) -> Result<Value, Error> {
    let GrepCall { pattern, flags } = call;
    if pattern.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::COMMAND_GREP_EMPTY_PATTERN).to_string(),
        ));
    }

    let mut args = collect_flag_args(flags)?;
    args.push(pattern.to_owned());
    let command = format_command("grep", &args)?;
    let input = to_bytes(value)?;

    #[cfg(windows)]
    let output = run_program("grep", &args, &input, context)
        .map_err(|err| command_error(err, state.name(), &command))?;

    #[cfg(not(windows))]
    let output = run_command(&command, &input, context)
        .map_err(|err| command_error(err, state.name(), &command))?;

    match output {
        StdoutResult::Bytes(bytes) => Ok(value_from_bytes(bytes)),
        StdoutResult::Tempfile(path) => Ok(Value::from(path.as_str())),
    }
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
                            localization::message(keys::COMMAND_GREP_FLAGS_NOT_STRING).to_string(),
                        ))
                    },
                    |s| Ok(s.to_owned()),
                )
            })
            .collect(),
        _ => value.as_str().map(|s| vec![s.to_owned()]).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::COMMAND_GREP_FLAGS_NOT_STRING).to_string(),
            )
        }),
    }
}

fn format_command(base: &str, args: &[String]) -> Result<String, Error> {
    let mut command = String::from(base);
    for arg in args {
        command.push(' ');
        let quoted = quote(arg).map_err(|err| {
            Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::COMMAND_QUOTE_INVALID)
                    .with_arg("arg", format!("{arg:?}"))
                    .with_arg("details", err.to_string())
                    .to_string(),
            )
        })?;
        command.push_str(&quoted);
    }
    Ok(command)
}

fn to_bytes(value: &Value) -> Result<Vec<u8>, Error> {
    if value.is_undefined() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::COMMAND_INPUT_UNDEFINED).to_string(),
        ));
    }

    if let Some(bytes) = value.as_bytes() {
        return Ok(bytes.to_vec());
    }

    Ok(value.to_string().into_bytes())
}
