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

mod config;
mod context;
mod error;
mod execution;
mod filters;
mod pipes;
mod quote;
mod result;

pub(super) use super::value_from_bytes;
pub(crate) use config::CommandConfig;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use config::CommandOptions;
use context::{CommandContext, GrepCall};
use filters::{execute_grep, execute_shell};
use minijinja::{Environment, State, value::Value};

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
pub(crate) fn register(env: &mut Environment<'_>, impure: Arc<AtomicBool>, config: CommandConfig) {
    let shared_config = Arc::new(config);
    let shell_flag = Arc::clone(&impure);
    let shell_config = Arc::clone(&shared_config);
    env.add_filter(
        "shell",
        move |state: &State, value: Value, command: String, options: Option<Value>| {
            shell_flag.store(true, Ordering::Relaxed);
            let parsed = CommandOptions::from_value(options)?;
            let context = CommandContext::new(Arc::clone(&shell_config), parsed);
            execute_shell(state, &value, &command, &context)
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
            execute_grep(state, &value, call, &context)
        },
    );
}
