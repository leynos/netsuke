//! Type-check the explicit Ninja environment API as an external embedder.
//!
//! `CommandEnv`, both request structs, and `run_ninja_with` and
//! `run_ninja_tool_with` must be reachable and composable from outside the
//! crate without touching the process environment.

use netsuke::runner::{
    BuildTargets, CommandEnv, NinjaBuildRequest, NinjaProcessOptions, NinjaToolRequest, StderrMode,
    Utf8Path, run_ninja_tool_with, run_ninja_with,
};
use std::io;

/// The pieces an embedder holds before building requests.
pub struct Parts<'a> {
    /// The Ninja executable to run.
    pub program: &'a Utf8Path,
    /// Process options shared by both requests.
    pub options: &'a NinjaProcessOptions,
    /// The generated build file.
    pub build_file: &'a Utf8Path,
    /// The targets the build request names.
    pub targets: &'a BuildTargets<'a>,
    /// The explicit child environment.
    pub env: &'a CommandEnv,
}

/// Compose an embedder's child environment from the builder surface.
#[must_use]
pub fn embedder_environment() -> CommandEnv {
    CommandEnv::inherit()
        .with_var("NINJA_STATUS", "[%f/%t] ")
        .with_path("/opt/toolchain/bin")
}

/// Report whether the composed environment carries the configured search path.
#[must_use]
pub fn has_search_path(env: &CommandEnv) -> bool {
    !env.is_empty() && env.get("PATH").is_some()
}

/// Construct both request bundles around one `CommandEnv`.
#[must_use]
pub const fn compose_requests<'a>(
    parts: &Parts<'a>,
) -> (NinjaBuildRequest<'a>, NinjaToolRequest<'a>) {
    let build = NinjaBuildRequest {
        program: parts.program,
        options: parts.options,
        build_file: parts.build_file,
        targets: parts.targets,
        env: parts.env,
        stderr_mode: StderrMode::Forward,
    };
    let tool = NinjaToolRequest {
        program: parts.program,
        options: parts.options,
        build_file: parts.build_file,
        tool: "clean",
        env: parts.env,
        stderr_mode: StderrMode::Forward,
    };
    (build, tool)
}

/// Drive both explicit entry points with composed requests.
///
/// # Errors
///
/// Returns the error either Ninja invocation reports.
pub fn drive_both_boundaries(parts: &Parts<'_>) -> io::Result<()> {
    let (build, tool) = compose_requests(parts);
    run_ninja_with(&build)?;
    run_ninja_tool_with(&tool)
}
