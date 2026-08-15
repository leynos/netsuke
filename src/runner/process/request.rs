//! Borrowed parameter bundles for the Ninja execution helpers.
//!
//! Separated from `process::mod` so that module stays under the repository's
//! 400-line ceiling. These are data, not behaviour: the request types describe
//! what an invocation needs; `mod` holds the functions that act on them.

use super::{BuildTargets, CommandEnv, StderrMode};
use camino::Utf8PathBuf;
use std::path::Path;

/// Process settings needed to configure a Ninja invocation.
#[derive(Debug, Clone, Default)]
pub struct NinjaProcessOptions {
    /// Optional UTF-8 working directory passed to the child process.
    pub working_dir: Option<Utf8PathBuf>,
    /// Optional maximum number of parallel Ninja jobs.
    pub jobs: Option<usize>,
}

/// Borrowed parameter bundle for `ninja` build execution helpers.
#[derive(Clone, Copy)]
pub struct NinjaBuildRequest<'a> {
    /// Ninja executable to invoke.
    pub program: &'a Path,
    /// Process settings supplying the working directory and job count.
    pub options: &'a NinjaProcessOptions,
    /// Generated build file passed with `-f`.
    pub build_file: &'a Path,
    /// Targets appended after the base flags.
    pub targets: &'a BuildTargets<'a>,
    /// Environment overrides applied to the child process. Use
    /// [`CommandEnv::inherit`] to leave the parent environment in place.
    pub env: &'a CommandEnv,
    /// Policy routing the child's standard streams. [`StderrMode::Suppress`]
    /// keeps JSON diagnostics machine-readable by draining both streams.
    pub stderr_mode: StderrMode,
}

/// Borrowed parameter bundle for `ninja -t` tool execution helpers.
#[derive(Clone, Copy)]
pub struct NinjaToolRequest<'a> {
    /// Ninja executable to invoke.
    pub program: &'a Path,
    /// Process settings supplying the working directory and job count.
    pub options: &'a NinjaProcessOptions,
    /// Generated build file passed with `-f`.
    pub build_file: &'a Path,
    /// Tool name passed to `ninja -t`.
    pub tool: &'a str,
    /// Environment overrides applied to the child process.
    pub env: &'a CommandEnv,
    /// Policy routing the child's standard streams. [`StderrMode::Suppress`]
    /// keeps JSON diagnostics machine-readable by draining both streams.
    pub stderr_mode: StderrMode,
}
