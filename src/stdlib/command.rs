//! Shell-oriented helpers for the `MiniJinja` standard library.
//!
//! The helpers bridge template values into the local shell while keeping
//! behaviour predictable across platforms. All helpers mark the stdlib state as
//! impure so the caller can invalidate any caching layer that depends on pure
//! template evaluation.
//!
//! # Output limits
//!
//! Capture and streaming helpers enforce configurable byte budgets to prevent
//! helpers from exhausting memory or disk space. Callers configure distinct
//! ceilings for `stdout` capture and streamed tempfile output through
//! `StdlibConfig`, and the runtime raises descriptive errors when a command
//! exceeds either limit.
//!
//! # Windows quoting strategy
//!
//! Windows does not ship a widely-used Rust crate that can reliably escape
//! `cmd.exe` arguments. General Windows bindings such as `winsafe` expose
//! Win32 APIs but leave command-line quoting to the caller, and no alternative
//! crate currently offers a robust drop-in solution. We therefore maintain a
//! small implementation derived from the official [`CommandLineToArgvW`][ms-argv]
//! documentation and the detailed guidance on [metacharacter handling][ss64]. The
//! routine emits double-quoted arguments when required and escapes
//! metacharacters (`^`, `&`, `|`, `<`, `>`, `%`, and `!`) so that `cmd.exe`
//! always treats templated data as literals. Double quotes within the argument
//! are escaped with a caret so the shell preserves them, while preceding
//! backslashes are passed through unchanged because `cmd.exe` does not treat `\`
//! as an escape character. Line-feed and carriage-return characters are rejected
//! outright because `cmd.exe` interprets them as command terminators even inside
//! quotes. When the command reaches the invoked program the Windows argument
//! splitter applies the [`CommandLineToArgvW`][ms-argv] backslash rules, so
//! sequences such as `\"` still deliver the intended backslashes alongside the
//! literal quote.
//!
//! [ms-argv]: https://learn.microsoft.com/windows/win32/api/shellapi/nf-shellapi-commandlinetoargvw
//! [ss64]: https://ss64.com/nt/syntax-esc.html
//!
//! # Security
//!
//! The `shell` and `grep` filters execute external commands based on template
//! content. Templates using these filters must come from trusted sources only.
//! Never allow untrusted input to control command strings or patterns, as this
//! enables arbitrary code execution.

use std::{
    fmt::{self},
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use super::{DEFAULT_COMMAND_TEMP_DIR, value_from_bytes};
use camino::Utf8PathBuf;
use cap_std::fs_utf8::Dir;
use minijinja::{
    Error, ErrorKind, State,
    value::{Value, ValueKind},
};
#[cfg(not(windows))]
use shell_quote::{QuoteRefExt, Sh};
use tempfile::{Builder, NamedTempFile};
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

/// Size of the temporary buffer used when draining child process pipes.
const PIPE_CHUNK_SIZE: usize = 8192;

#[derive(Clone)]
pub(crate) struct CommandConfig {
    pub(crate) max_capture_bytes: u64,
    pub(crate) max_stream_bytes: u64,
    temp_dir: CommandTempDir,
}

impl CommandConfig {
    pub(crate) fn new(
        max_capture_bytes: u64,
        max_stream_bytes: u64,
        workspace_root: Arc<Dir>,
        workspace_root_path: Option<Arc<Utf8PathBuf>>,
    ) -> Self {
        Self {
            max_capture_bytes,
            max_stream_bytes,
            temp_dir: CommandTempDir::new(workspace_root, workspace_root_path),
        }
    }

    fn create_tempfile(&self, label: &str) -> io::Result<CommandTempFile> {
        self.temp_dir.create(label)
    }
}

#[derive(Clone)]
struct CommandTempDir {
    workspace_root: Arc<Dir>,
    workspace_root_path: Option<Arc<Utf8PathBuf>>,
    relative: Utf8PathBuf,
}

impl CommandTempDir {
    fn new(workspace_root: Arc<Dir>, workspace_root_path: Option<Arc<Utf8PathBuf>>) -> Self {
        Self {
            workspace_root,
            workspace_root_path,
            relative: Utf8PathBuf::from(DEFAULT_COMMAND_TEMP_DIR),
        }
    }

    fn create(&self, label: &str) -> io::Result<CommandTempFile> {
        self.workspace_root.create_dir_all(&self.relative)?;
        let mut builder = Builder::new();
        builder.prefix(label);
        let file = if let Some(root_path) = &self.workspace_root_path {
            let dir_path = root_path.join(&self.relative);
            fs::create_dir_all(dir_path.as_std_path())?;
            builder.tempfile_in(dir_path.as_std_path())?
        } else {
            builder.tempfile()?
        };
        Ok(CommandTempFile { file })
    }
}

struct CommandTempFile {
    file: NamedTempFile,
}

impl CommandTempFile {
    fn into_file(self) -> NamedTempFile {
        self.file
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputMode {
    Capture,
    Tempfile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputStream {
    Stdout,
    Stderr,
}

impl OutputStream {
    const fn describe(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }

    const fn tempfile_label(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }

    const fn empty_tempfile_label(self) -> &'static str {
        match self {
            Self::Stdout => "stdout-empty",
            Self::Stderr => "stderr-empty",
        }
    }
}

impl OutputMode {
    const fn describe(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Tempfile => "streaming",
        }
    }
}

#[derive(Clone, Copy)]
struct PipeSpec {
    stream: OutputStream,
    mode: OutputMode,
    limit: u64,
}

impl PipeSpec {
    const fn new(stream: OutputStream, mode: OutputMode, limit: u64) -> Self {
        Self {
            stream,
            mode,
            limit,
        }
    }

    const fn stream(self) -> OutputStream {
        self.stream
    }

    const fn mode(self) -> OutputMode {
        self.mode
    }

    const fn limit(self) -> u64 {
        self.limit
    }

    const fn into_limit(self) -> PipeLimit {
        PipeLimit {
            spec: self,
            consumed: 0,
        }
    }
}

struct PipeLimit {
    spec: PipeSpec,
    consumed: u64,
}

impl PipeLimit {
    fn record(&mut self, read: usize) -> Result<(), CommandFailure> {
        let bytes = u64::try_from(read)
            .map_err(|_| CommandFailure::Io(io::Error::other("pipe read size overflow")))?;
        let new_total = self
            .consumed
            .checked_add(bytes)
            .ok_or_else(|| CommandFailure::Io(io::Error::other("pipe output size overflow")))?;
        if new_total > self.spec.limit() {
            return Err(CommandFailure::OutputLimit {
                stream: self.spec.stream(),
                mode: self.spec.mode(),
                limit: self.spec.limit(),
            });
        }
        self.consumed = new_total;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CommandOptions {
    stdout_mode: OutputMode,
}

impl CommandOptions {
    fn from_value(options: Option<Value>) -> Result<Self, Error> {
        let Some(raw) = options else {
            return Ok(Self::default());
        };

        if raw.is_undefined() {
            return Ok(Self::default());
        }

        match raw.kind() {
            ValueKind::String => {
                let Some(text) = raw.as_str() else {
                    return Err(Error::new(
                        ErrorKind::InvalidOperation,
                        "command options string must be valid UTF-8",
                    ));
                };
                Self::from_mode_str(text)
            }
            ValueKind::Map | ValueKind::Plain => {
                let mode_value = raw.get_attr("mode")?;
                if mode_value.is_undefined() {
                    return Ok(Self::default());
                }
                let Some(mode) = mode_value.as_str() else {
                    return Err(Error::new(
                        ErrorKind::InvalidOperation,
                        "command option 'mode' must be a string",
                    ));
                };
                Self::from_mode_str(mode)
            }
            _ => Err(Error::new(
                ErrorKind::InvalidOperation,
                "command options must be a string or mapping",
            )),
        }
    }

    fn from_mode_str(mode: &str) -> Result<Self, Error> {
        match mode {
            "capture" => Ok(Self {
                stdout_mode: OutputMode::Capture,
            }),
            "tempfile" | "stream" | "streaming" => Ok(Self {
                stdout_mode: OutputMode::Tempfile,
            }),
            other => Err(Error::new(
                ErrorKind::InvalidOperation,
                format!("unsupported command output mode '{other}'"),
            )),
        }
    }

    const fn stdout_mode(self) -> OutputMode {
        self.stdout_mode
    }
}

impl Default for CommandOptions {
    fn default() -> Self {
        Self {
            stdout_mode: OutputMode::Capture,
        }
    }
}

#[derive(Debug)]
enum StdoutResult {
    Bytes(Vec<u8>),
    Tempfile(Utf8PathBuf),
}

#[derive(Debug)]
enum PipeOutcome {
    Bytes(Vec<u8>),
    Tempfile(Utf8PathBuf),
}

#[derive(Clone)]
struct CommandContext {
    config: Arc<CommandConfig>,
    options: CommandOptions,
}

impl CommandContext {
    fn new(config: Arc<CommandConfig>, options: CommandOptions) -> Self {
        Self { config, options }
    }

    const fn stdout_mode(&self) -> OutputMode {
        self.options.stdout_mode()
    }

    fn config(&self) -> &CommandConfig {
        &self.config
    }

    fn config_handle(&self) -> Arc<CommandConfig> {
        Arc::clone(&self.config)
    }
}

struct GrepCall<'a> {
    pattern: &'a str,
    flags: Option<Value>,
}

impl<'a> GrepCall<'a> {
    const fn new(pattern: &'a str, flags: Option<Value>) -> Self {
        Self { pattern, flags }
    }
}

#[derive(Clone, Copy)]
struct CommandLocation<'a> {
    template: &'a str,
    command: &'a str,
}

impl<'a> CommandLocation<'a> {
    const fn new(template: &'a str, command: &'a str) -> Self {
        Self { template, command }
    }

    fn describe(self) -> String {
        format!("command '{}' in template '{}'", self.command, self.template)
    }
}

#[derive(Clone, Copy)]
struct ExitDetails<'a> {
    status: Option<i32>,
    stderr: &'a [u8],
}

impl<'a> ExitDetails<'a> {
    const fn new(status: Option<i32>, stderr: &'a [u8]) -> Self {
        Self { status, stderr }
    }
}

#[derive(Clone, Copy)]
struct LimitExceeded {
    stream: OutputStream,
    mode: OutputMode,
    limit: u64,
}

impl LimitExceeded {
    const fn new(stream: OutputStream, mode: OutputMode, limit: u64) -> Self {
        Self {
            stream,
            mode,
            limit,
        }
    }
}

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
pub(crate) fn register(
    env: &mut minijinja::Environment<'_>,
    impure: Arc<AtomicBool>,
    config: CommandConfig,
) {
    let shared_config = Arc::new(config);
    let shell_flag = Arc::clone(&impure);
    let shell_config = Arc::clone(&shared_config);
    env.add_filter(
        "shell",
        move |state: &State, value: Value, command: String, options: Option<Value>| {
            shell_flag.store(true, Ordering::Relaxed);
            let parsed = CommandOptions::from_value(options)?;
            let context = CommandContext::new(Arc::clone(&shell_config), parsed);
            execute_shell(state, &value, &command, context)
        },
    );

    let grep_flag = impure;
    let grep_config = Arc::clone(&shared_config);
    env.add_filter(
        "grep",
        move |state: &State,
              value: Value,
              pattern: String,
              flags: Option<Value>,
              options: Option<Value>| {
            grep_flag.store(true, Ordering::Relaxed);
            let parsed = CommandOptions::from_value(options)?;
            let context = CommandContext::new(Arc::clone(&grep_config), parsed);
            let call = GrepCall::new(&pattern, flags);
            execute_grep(state, &value, call, context)
        },
    );
}

fn execute_shell(
    state: &State,
    value: &Value,
    command: &str,
    context: CommandContext,
) -> Result<Value, Error> {
    let cmd = command.trim();
    if cmd.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "shell filter requires a non-empty command",
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

fn execute_grep(
    state: &State,
    value: &Value,
    call: GrepCall<'_>,
    context: CommandContext,
) -> Result<Value, Error> {
    let GrepCall { pattern, flags } = call;
    if pattern.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "grep filter requires a search pattern",
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

fn format_command(base: &str, args: &[String]) -> Result<String, Error> {
    let mut command = String::from(base);
    for arg in args {
        command.push(' ');
        let quoted = quote(arg).map_err(|err| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("argument {arg:?} cannot be safely quoted: {err}"),
            )
        })?;
        command.push_str(&quoted);
    }
    Ok(command)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum QuoteError {
    ContainsLineBreak,
}

impl fmt::Display for QuoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContainsLineBreak => f.write_str(
                "arguments containing carriage returns or line feeds cannot be safely quoted",
            ),
        }
    }
}

#[cfg(windows)]
fn quote(arg: &str) -> Result<String, QuoteError> {
    // cmd.exe interprets metacharacters even inside double quotes. Escape them using
    // caret prefixes and collapse environment-expansion tokens so arbitrary inputs
    // remain literal. Reference: https://ss64.com/nt/syntax-esc.html
    // Line breaks still terminate the command even when quoted, so reject them to
    // keep templated arguments from spawning additional statements.
    if arg.chars().any(|ch| matches!(ch, '\n' | '\r')) {
        return Err(QuoteError::ContainsLineBreak);
    }

    if arg.is_empty() {
        return Ok("\"\"".to_owned());
    }

    let needs_quotes = arg.chars().any(|ch| {
        matches!(
            ch,
            ' ' | '\t' | '"' | '^' | '&' | '|' | '<' | '>' | '%' | '!'
        )
    });
    if !needs_quotes {
        return Ok(arg.to_owned());
    }

    let mut buf = String::with_capacity(arg.len() + 2);
    buf.push('"');
    for ch in arg.chars() {
        match ch {
            '"' => {
                buf.push('^');
                buf.push('"');
            }
            '^' | '&' | '|' | '<' | '>' => {
                buf.push('^');
                buf.push(ch);
            }
            '%' => {
                buf.push('%');
                buf.push('%');
            }
            '!' => {
                buf.push('^');
                buf.push('!');
            }
            _ => buf.push(ch),
        }
    }
    buf.push('"');
    Ok(buf)
}

#[cfg(not(windows))]
fn quote(arg: &str) -> Result<String, QuoteError> {
    if arg.chars().any(|ch| matches!(ch, '\n' | '\r')) {
        return Err(QuoteError::ContainsLineBreak);
    }

    let bytes = arg.quoted(Sh);
    match String::from_utf8(bytes) {
        Ok(text) => Ok(text),
        Err(err) => {
            debug_assert!(false, "quoted args must be valid UTF-8: {err}");
            Ok(String::from_utf8_lossy(err.as_bytes()).into_owned())
        }
    }
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

fn run_command(
    command: &str,
    input: &[u8],
    context: CommandContext,
) -> Result<StdoutResult, CommandFailure> {
    let mut cmd = Command::new(SHELL);
    cmd.args(SHELL_ARGS)
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    run_child(cmd, input, context)
}

#[cfg(windows)]
fn run_program(
    program: &str,
    args: &[String],
    input: &[u8],
    context: CommandContext,
) -> Result<StdoutResult, CommandFailure> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    run_child(cmd, input, context)
}

fn run_child(
    mut command: Command,
    input: &[u8],
    context: CommandContext,
) -> Result<StdoutResult, CommandFailure> {
    let mut child = command.spawn().map_err(CommandFailure::Spawn)?;
    let mut stdin_handle = child.stdin.take().map(|mut stdin| {
        let buffer = input.to_vec();
        thread::spawn(move || stdin.write_all(&buffer))
    });

    let stdout_limit = match context.stdout_mode() {
        OutputMode::Capture => context.config().max_capture_bytes,
        OutputMode::Tempfile => context.config().max_stream_bytes,
    };
    let stderr_limit = context.config().max_capture_bytes;

    let stdout_spec = PipeSpec::new(OutputStream::Stdout, context.stdout_mode(), stdout_limit);
    let stderr_spec = PipeSpec::new(OutputStream::Stderr, OutputMode::Capture, stderr_limit);

    let stdout_config = context.config_handle();
    let stderr_config = context.config_handle();

    let mut stdout_reader =
        spawn_pipe_reader(child.stdout.take(), stdout_spec, Arc::clone(&stdout_config));
    let mut stderr_reader =
        spawn_pipe_reader(child.stderr.take(), stderr_spec, Arc::clone(&stderr_config));

    let status = match wait_for_exit(&mut child, COMMAND_TIMEOUT) {
        Ok(status) => status,
        Err(err) => {
            cleanup_readers(&mut stdout_reader, &mut stderr_reader, &mut stdin_handle);
            return Err(err);
        }
    };

    let stdout = join_reader(stdout_reader.take(), stdout_spec, stdout_config)?;
    let stderr_outcome = join_reader(stderr_reader.take(), stderr_spec, stderr_config)?;

    let stderr = match stderr_outcome {
        PipeOutcome::Bytes(bytes) => bytes,
        PipeOutcome::Tempfile(path) => {
            tracing::warn!(?path, "stderr reader returned a temp file; discarding path");
            Vec::new()
        }
    };

    handle_stdin_result(stdin_handle.take(), status.code(), &stderr)?;

    if status.success() {
        Ok(match stdout {
            PipeOutcome::Bytes(bytes) => StdoutResult::Bytes(bytes),
            PipeOutcome::Tempfile(path) => StdoutResult::Tempfile(path),
        })
    } else {
        Err(CommandFailure::Exit {
            status: status.code(),
            stderr,
        })
    }
}

fn cleanup_readers(
    stdout_reader: &mut Option<thread::JoinHandle<Result<PipeOutcome, CommandFailure>>>,
    stderr_reader: &mut Option<thread::JoinHandle<Result<PipeOutcome, CommandFailure>>>,
    stdin_handle: &mut Option<thread::JoinHandle<io::Result<()>>>,
) {
    join_pipe_for_cleanup("stdout", stdout_reader);
    join_pipe_for_cleanup("stderr", stderr_reader);
    if let Some(handle) = stdin_handle.take()
        && let Err(join_err) = handle.join()
    {
        tracing::warn!("stdin writer thread panicked: {join_err:?}");
    }
}

fn join_pipe_for_cleanup(
    label: &str,
    reader_handle: &mut Option<thread::JoinHandle<Result<PipeOutcome, CommandFailure>>>,
) {
    if let Some(join_handle) = reader_handle.take() {
        match join_handle.join() {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => {
                tracing::warn!(stream = label, ?err, "pipe reader failed during cleanup");
            }
            Err(join_err) => {
                tracing::warn!(stream = label, ?join_err, "pipe reader thread panicked");
            }
        }
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
    let location = CommandLocation::new(template, command);
    match err {
        CommandFailure::Spawn(spawn) => spawn_error(location, &spawn),
        CommandFailure::Io(io_err) => io_error(location, &io_err),
        CommandFailure::BrokenPipe {
            source,
            status,
            stderr,
        } => broken_pipe_error(location, &source, ExitDetails::new(status, &stderr)),
        CommandFailure::Exit { status, stderr } => {
            exit_error(location, ExitDetails::new(status, &stderr))
        }
        CommandFailure::OutputLimit {
            stream,
            mode,
            limit,
        } => output_limit_error(location, LimitExceeded::new(stream, mode, limit)),
        CommandFailure::StreamPathNotUtf8(path) => stream_path_error(location, &path),
        CommandFailure::Timeout(duration) => timeout_error(location, duration),
    }
}

fn spawn_error(location: CommandLocation<'_>, err: &io::Error) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("failed to spawn {}: {err}", location.describe()),
    )
}

fn io_error(location: CommandLocation<'_>, err: &io::Error) -> Error {
    let mut message = format!("{} failed: {err}", location.describe());
    if err.kind() == io::ErrorKind::BrokenPipe {
        message.push_str(" (command closed input early)");
    }
    Error::new(ErrorKind::InvalidOperation, message)
}

fn broken_pipe_error(
    location: CommandLocation<'_>,
    err: &io::Error,
    details: ExitDetails<'_>,
) -> Error {
    let mut message = format!(
        "{} failed: {err} (command closed input early)",
        location.describe()
    );
    append_exit_status(&mut message, details.status);
    append_stderr(&mut message, details.stderr);
    Error::new(ErrorKind::InvalidOperation, message)
}

fn exit_error(location: CommandLocation<'_>, details: ExitDetails<'_>) -> Error {
    let mut message = details.status.map_or_else(
        || format!("{} terminated by signal", location.describe()),
        |code| format!("{} exited with status {code}", location.describe()),
    );
    append_stderr(&mut message, details.stderr);
    Error::new(ErrorKind::InvalidOperation, message)
}

fn output_limit_error(location: CommandLocation<'_>, exceeded: LimitExceeded) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!(
            "{} exceeded {stream} {mode} limit of {limit} bytes",
            location.describe(),
            stream = exceeded.stream.describe(),
            mode = exceeded.mode.describe(),
            limit = exceeded.limit,
        ),
    )
}

fn stream_path_error(location: CommandLocation<'_>, path: &Path) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!(
            "{} produced a temporary output path that is not valid UTF-8: {}",
            location.describe(),
            path.display()
        ),
    )
}

fn timeout_error(location: CommandLocation<'_>, duration: Duration) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!(
            "{} timed out after {}s",
            location.describe(),
            duration.as_secs()
        ),
    )
}

fn append_exit_status(message: &mut String, status: Option<i32>) {
    if let Some(code) = status {
        message.push_str("; exited with status ");
        let code_text = code.to_string();
        message.push_str(&code_text);
    } else {
        message.push_str("; terminated by signal");
    }
}

#[derive(Debug)]
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
    OutputLimit {
        stream: OutputStream,
        mode: OutputMode,
        limit: u64,
    },
    StreamPathNotUtf8(PathBuf),
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
        if let Err(err) = child.wait() {
            tracing::warn!("failed to reap timed-out command: {err}");
        }
        Err(CommandFailure::Timeout(timeout))
    }
}

fn spawn_pipe_reader<R>(
    pipe: Option<R>,
    spec: PipeSpec,
    config: Arc<CommandConfig>,
) -> Option<thread::JoinHandle<Result<PipeOutcome, CommandFailure>>>
where
    R: Read + Send + 'static,
{
    pipe.map(|reader| thread::spawn(move || read_pipe(reader, spec, config)))
}

fn join_reader(
    reader_handle: Option<thread::JoinHandle<Result<PipeOutcome, CommandFailure>>>,
    spec: PipeSpec,
    config: Arc<CommandConfig>,
) -> Result<PipeOutcome, CommandFailure> {
    match reader_handle {
        Some(join_handle) => join_handle
            .join()
            .map_err(|_| CommandFailure::Io(io::Error::other("pipe reader panicked")))?,
        None => {
            if matches!(spec.mode(), OutputMode::Tempfile) {
                create_empty_tempfile(&config, spec.stream().empty_tempfile_label())
                    .map(PipeOutcome::Tempfile)
            } else {
                Ok(PipeOutcome::Bytes(Vec::new()))
            }
        }
    }
}

/// Drains a child process pipe according to the provided `PipeSpec`, enforcing
/// the configured byte limit and producing either in-memory bytes or a
/// tempfile-backed outcome.
fn read_pipe<R>(
    reader: R,
    spec: PipeSpec,
    config: Arc<CommandConfig>,
) -> Result<PipeOutcome, CommandFailure>
where
    R: Read,
{
    let limit = spec.into_limit();
    match spec.mode() {
        OutputMode::Capture => read_pipe_capture(reader, limit),
        OutputMode::Tempfile => read_pipe_tempfile(
            reader,
            limit,
            spec.stream().tempfile_label(),
            config.as_ref(),
        ),
    }
}

/// Reads a pipe into memory while enforcing the capture byte limit recorded in
/// the `PipeLimit` tracker.
fn read_pipe_capture<R>(mut reader: R, mut limit: PipeLimit) -> Result<PipeOutcome, CommandFailure>
where
    R: Read,
{
    let mut buf = Vec::new();
    let mut chunk = [0_u8; PIPE_CHUNK_SIZE];
    loop {
        let read = reader.read(&mut chunk).map_err(CommandFailure::Io)?;
        if read == 0 {
            break;
        }
        limit.record(read)?;
        buf.extend(chunk.iter().take(read).copied());
    }
    Ok(PipeOutcome::Bytes(buf))
}

/// Streams a pipe into a tempfile rooted within the workspace, enforcing the
/// streaming byte limit recorded in `PipeLimit` and returning the final path.
fn read_pipe_tempfile<R>(
    mut reader: R,
    mut limit: PipeLimit,
    label: &str,
    config: &CommandConfig,
) -> Result<PipeOutcome, CommandFailure>
where
    R: Read,
{
    let tempfile = config.create_tempfile(label).map_err(CommandFailure::Io)?;
    let mut file = tempfile.into_file();
    let mut chunk = [0_u8; PIPE_CHUNK_SIZE];
    loop {
        let read = reader.read(&mut chunk).map_err(CommandFailure::Io)?;
        if read == 0 {
            break;
        }
        limit.record(read)?;
        file.write_all(&chunk[..read]).map_err(CommandFailure::Io)?;
    }
    file.flush().map_err(CommandFailure::Io)?;
    let temp_path = file.into_temp_path();
    let path = temp_path
        .keep()
        .map_err(|err| CommandFailure::Io(err.error))?;
    let utf8 = Utf8PathBuf::from_path_buf(path).map_err(CommandFailure::StreamPathNotUtf8)?;
    Ok(PipeOutcome::Tempfile(utf8))
}

fn create_empty_tempfile(
    config: &CommandConfig,
    label: &str,
) -> Result<Utf8PathBuf, CommandFailure> {
    let tempfile = config.create_tempfile(label).map_err(CommandFailure::Io)?;
    let file = tempfile.into_file();
    let path = file
        .into_temp_path()
        .keep()
        .map_err(|err| CommandFailure::Io(err.error))?;
    Utf8PathBuf::from_path_buf(path).map_err(CommandFailure::StreamPathNotUtf8)
}

fn append_stderr(message: &mut String, stderr: &[u8]) {
    let stderr_text = String::from_utf8_lossy(stderr);
    let trimmed = stderr_text.trim();
    if !trimmed.is_empty() {
        message.push_str(": ");
        message.push_str(trimmed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::{DEFAULT_COMMAND_MAX_OUTPUT_BYTES, DEFAULT_COMMAND_MAX_STREAM_BYTES};
    use camino::Utf8PathBuf;
    use cap_std::{ambient_authority, fs_utf8::Dir};
    use std::{fs, io::Cursor};
    use tempfile::tempdir;

    #[cfg(windows)]
    use anyhow::{Result, ensure};

    fn test_command_config() -> (tempfile::TempDir, CommandConfig) {
        let temp = tempdir().expect("create command temp workspace");
        let path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .expect("temp workspace should be valid UTF-8");
        let dir =
            Dir::open_ambient_dir(&path, ambient_authority()).expect("open temp workspace dir");
        let config = CommandConfig::new(
            DEFAULT_COMMAND_MAX_OUTPUT_BYTES,
            DEFAULT_COMMAND_MAX_STREAM_BYTES,
            Arc::new(dir),
            Some(Arc::new(path)),
        );
        (temp, config)
    }

    #[cfg(windows)]
    #[test]
    fn quote_escapes_cmd_metacharacters() -> Result<()> {
        let success_cases = [
            ("simple", "simple"),
            ("", "\"\""),
            ("needs space", "\"needs space\""),
            ("pipe|test", "\"pipe^|test\""),
            ("redir<test", "\"redir^<test\""),
            ("redir>test", "\"redir^>test\""),
            ("caret^test", "\"caret^^test\""),
            ("tab\ttab", "\"tab\ttab\""),
            ("report&del *.txt", "\"report^&del *.txt\""),
            ("%TEMP%", "\"%%TEMP%%\""),
            ("echo!boom", "\"echo^!boom\""),
            ("say \"hi\"", "\"say ^\"hi^\"\""),
            ("\"", "\"^\"\""),
            ("foo\"bar\"baz", "\"foo^\"bar^\"baz\""),
            ("!DELAYED!", "\"^!DELAYED^!\""),
            ("\"!VAR!\"", "\"^\"^!VAR^!^\"\""),
            (r#"C:\path\"ending"#, r#""C:\path\^"ending""#),
        ];

        for (input, expected) in success_cases {
            let actual = quote(input)?;
            ensure!(
                actual == expected,
                "quote({input:?}) -> {actual:?}, expected {expected:?}"
            );
        }

        let error_cases = [
            ("line\nbreak", QuoteError::ContainsLineBreak),
            ("carriage\rreturn", QuoteError::ContainsLineBreak),
        ];

        for (input, expected) in error_cases {
            let err = quote(input).expect_err(&format!(
                "quote({input:?}) succeeded but expected error {expected:?}"
            ));
            ensure!(
                err == expected,
                "quote({input:?}) returned error {err:?}, expected {expected:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn read_pipe_capture_collects_bytes_within_limit() {
        let data = b"payload".to_vec();
        let outcome = read_pipe_capture(
            Cursor::new(data.clone()),
            PipeSpec::new(OutputStream::Stdout, OutputMode::Capture, 128).into_limit(),
        )
        .expect("capture should succeed within the configured limit");
        match outcome {
            PipeOutcome::Bytes(buf) => assert_eq!(buf, data),
            PipeOutcome::Tempfile(_) => panic!("capture mode should emit bytes"),
        }
    }

    #[test]
    fn read_pipe_capture_reports_limit_exceedance() {
        let err = read_pipe_capture(
            Cursor::new(vec![0_u8; 16]),
            PipeSpec::new(OutputStream::Stdout, OutputMode::Capture, 8).into_limit(),
        )
        .expect_err("capture should fail when it exceeds the configured limit");
        match err {
            CommandFailure::OutputLimit {
                stream,
                mode,
                limit,
            } => {
                assert_eq!(stream, OutputStream::Stdout);
                assert_eq!(mode, OutputMode::Capture);
                assert_eq!(limit, 8);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn read_pipe_tempfile_writes_streamed_data() {
        let payload = vec![b'x'; 32];
        let (_temp_dir, config) = test_command_config();
        let outcome = read_pipe_tempfile(
            Cursor::new(payload.clone()),
            PipeSpec::new(OutputStream::Stdout, OutputMode::Tempfile, 64).into_limit(),
            "stdout",
            &config,
        )
        .expect("streaming should succeed within the configured limit");
        let path = match outcome {
            PipeOutcome::Tempfile(path) => path,
            PipeOutcome::Bytes(_) => panic!("streaming mode should emit a tempfile path"),
        };
        let disk = fs::read(path.as_std_path()).expect("read streamed output");
        assert_eq!(disk, payload);
        fs::remove_file(path.as_std_path()).expect("cleanup streamed file");
    }

    #[test]
    fn read_pipe_tempfile_respects_stream_limit() {
        let (_temp_dir, config) = test_command_config();
        let err = read_pipe_tempfile(
            Cursor::new(vec![b'y'; 32]),
            PipeSpec::new(OutputStream::Stdout, OutputMode::Tempfile, 8).into_limit(),
            "stdout",
            &config,
        )
        .expect_err("streaming should fail when it exceeds the configured limit");
        match err {
            CommandFailure::OutputLimit {
                stream,
                mode,
                limit,
            } => {
                assert_eq!(stream, OutputStream::Stdout);
                assert_eq!(mode, OutputMode::Tempfile);
                assert_eq!(limit, 8);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
