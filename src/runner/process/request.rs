//! Borrowed parameter bundles for the Ninja execution helpers.
//!
//! Separated from `process::mod` so that module stays under the repository's
//! 400-line ceiling. These are data, not behaviour: the request types describe
//! what an invocation needs; `mod` holds the functions that act on them.

use super::{BuildTargets, CommandEnv};
use crate::cli::Cli;
use std::path::Path;

/// Borrowed parameter bundle for `ninja` build execution helpers.
#[derive(Clone, Copy)]
pub struct NinjaBuildRequest<'a> {
    /// Ninja executable to invoke.
    pub program: &'a Path,
    /// Parsed CLI settings supplying the working directory, job count, and
    /// diagnostics mode.
    pub cli: &'a Cli,
    /// Generated build file passed with `-f`.
    pub build_file: &'a Path,
    /// Targets appended after the base flags.
    pub targets: &'a BuildTargets<'a>,
    /// Environment overrides applied to the child process. Use
    /// [`CommandEnv::inherit`] to leave the parent environment in place.
    pub env: &'a CommandEnv,
}

/// Borrowed parameter bundle for `ninja -t` tool execution helpers.
#[derive(Clone, Copy)]
pub struct NinjaToolRequest<'a> {
    /// Ninja executable to invoke.
    pub program: &'a Path,
    /// Parsed CLI settings.
    pub cli: &'a Cli,
    /// Generated build file passed with `-f`.
    pub build_file: &'a Path,
    /// Tool name passed to `ninja -t`.
    pub tool: &'a str,
    /// Environment overrides applied to the child process.
    pub env: &'a CommandEnv,
}
