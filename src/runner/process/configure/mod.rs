//! Construction of the Ninja child command.
//!
//! Separated from `process::mod` so command shaping — working directory, job
//! count, build file, streaming pipes, and the injected environment — sits
//! together, and so neither module exceeds the repository's file-length limit.

use std::io;
use std::process::{Command, Stdio};

use camino::Utf8Path;

use super::{
    CommandEnv, NinjaBuildRequest, NinjaProcessOptions, NinjaToolRequest, canonicalize_utf8_path,
};

/// Configure the base Ninja command with working directory, job count, and build file.
///
/// Sets up stdout/stderr pipes for streaming. Callers append targets or tool
/// flags after this function returns.
///
/// # Errors
///
/// Returns an I/O error only when working-directory canonicalization fails.
/// Build and tool helpers tolerate build-file canonicalization failure by
/// passing the original UTF-8 path to Ninja.
fn configure_ninja_base(
    cmd: &mut Command,
    options: &NinjaProcessOptions,
    build_file: &Utf8Path,
    env: &CommandEnv,
) -> io::Result<()> {
    env.apply(cmd);
    if let Some(dir) = &options.working_dir {
        let canonical = canonicalize_utf8_path(dir)?;
        cmd.current_dir(canonical.as_std_path());
    }
    if let Some(jobs) = options.jobs {
        cmd.arg("-j").arg(jobs.to_string());
    }
    let build_file_path = canonicalize_utf8_path(build_file).unwrap_or_else(|_| build_file.into());
    cmd.arg("-f").arg(build_file_path.as_std_path());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    Ok(())
}

/// Configure a Ninja build command from `request`, appending its targets.
///
/// # Errors
///
/// Build-file canonicalization failure is tolerated by passing the original
/// UTF-8 path. Returns an error when working-directory canonicalization fails.
pub(super) fn configure_ninja_build_command(
    cmd: &mut Command,
    request: &NinjaBuildRequest<'_>,
) -> io::Result<()> {
    configure_ninja_base(cmd, request.options, request.build_file, request.env)?;
    let targets = request.targets;
    if !targets.as_slice().is_empty() {
        cmd.arg("--");
    }
    cmd.args(targets.as_slice());
    Ok(())
}

/// Configure a Ninja tool command from `request`, appending the tool name.
///
/// # Errors
///
/// Build-file canonicalization failure is tolerated by passing the original
/// UTF-8 path. Returns an error when working-directory canonicalization fails.
pub(super) fn configure_ninja_tool_command(
    cmd: &mut Command,
    request: &NinjaToolRequest<'_>,
) -> io::Result<()> {
    configure_ninja_base(cmd, request.options, request.build_file, request.env)?;
    cmd.arg("-t").arg(request.tool);
    Ok(())
}

#[cfg(test)]
mod tests;
