//! Test-only fixtures that build runner diagnostics through the real mapper.
//!
//! The human and JSON renderer tests both promise that a Ninja diagnostic does
//! not inherit the invocation context. Constructing `RunnerError` values by
//! hand lets both renderers keep passing if [`ninja_failure`] regresses to
//! attaching that context, so the fixtures here start from a typed invocation
//! failure and pass it through the production mapper. The sentinel context
//! stands in for the temporary build file, which makes any such regression
//! visible in both renderers.

use super::failure_diagnostic::ninja_failure;
use super::process::NinjaInvocationError;
use super::{NinjaExitError, RunnerError};
use anyhow::Result;
use camino::Utf8Path;
use std::io;
use std::process::ExitStatus;

/// Invocation context the mapper must not use for a classified failure.
const SENTINEL_CONTEXT: &str = "running ninja with build file /tmp/netsuke.SENTINEL.ninja";

/// Build a launch failure for a Ninja that is not installed.
///
/// # Errors
///
/// Returns an error if the mapper produces an unclassified failure rather than
/// the runner diagnostic the fixture asserts on.
pub(crate) fn ninja_spawn_failure() -> Result<RunnerError> {
    mapped(NinjaInvocationError::spawn(
        Utf8Path::new("ninja"),
        io::Error::new(
            io::ErrorKind::NotFound,
            "No such file or directory (os error 2)",
        ),
    ))
}

/// Build an unsuccessful exit carrying command-list attribution.
///
/// # Errors
///
/// Returns an error if the mapper produces an unclassified failure rather than
/// the runner diagnostic the fixture asserts on.
pub(crate) fn ninja_exit_failure(status: ExitStatus) -> Result<RunnerError> {
    let exit = NinjaExitError::new(status, "build", Some(String::from("action 0123, entry 2")));
    mapped(NinjaInvocationError::exit(exit))
}

/// Pass a typed failure through the production mapper.
///
/// The mapper must return the diagnostic itself, not a context wrapper around
/// it: a wrapper would carry the temporary build file into the summary. The
/// downcast alone would strip any wrapper and hide that regression, so the
/// check inspects the head of the chain first.
fn mapped(failure: NinjaInvocationError) -> Result<RunnerError> {
    let error = ninja_failure(failure, || SENTINEL_CONTEXT.to_owned());
    if !matches!(
        error.chain().next(),
        Some(first) if first.downcast_ref::<RunnerError>().is_some()
    ) {
        anyhow::bail!("the mapper must return the runner diagnostic without an outer context");
    }
    error.downcast::<RunnerError>().map_err(|unclassified| {
        unclassified.context("the fixture should produce a classified runner diagnostic")
    })
}
