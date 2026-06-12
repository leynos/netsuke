//! Construction of the Ninja child command.
//!
//! Separated from `process::mod` so command shaping — working directory, job
//! count, build file, streaming pipes, and the injected environment — sits
//! together, and so neither module exceeds the repository's file-length limit.

use std::io::{self, ErrorKind};
use std::path::Path;
use std::process::{Command, Stdio};

use camino::Utf8PathBuf;

use super::{
    CommandEnv, NinjaBuildRequest, NinjaProcessOptions, NinjaToolRequest, canonicalize_utf8_path,
};

/// Configure the base Ninja command with working directory, job count, and build file.
///
/// Sets up stdout/stderr pipes for streaming. Callers append targets or tool
/// flags after this function returns.
fn configure_ninja_base(
    cmd: &mut Command,
    options: &NinjaProcessOptions,
    build_file: &Path,
    env: &CommandEnv,
) -> io::Result<()> {
    env.apply(cmd);
    if let Some(dir) = &options.working_dir {
        let canonical = canonicalize_utf8_path(dir.as_path())?;
        cmd.current_dir(canonical.as_std_path());
    }
    if let Some(jobs) = options.jobs {
        cmd.arg("-j").arg(jobs.to_string());
    }
    let build_file_path = canonicalize_utf8_path(build_file).or_else(|_| {
        Utf8PathBuf::from_path_buf(build_file.to_path_buf()).map_err(|_| {
            io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "build file path {} is not valid UTF-8",
                    build_file.display()
                ),
            )
        })
    })?;
    cmd.arg("-f").arg(build_file_path.as_std_path());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    Ok(())
}

pub(super) fn configure_ninja_build_command(
    cmd: &mut Command,
    request: &NinjaBuildRequest<'_>,
) -> io::Result<()> {
    configure_ninja_base(cmd, request.options, request.build_file, request.env)?;
    let targets = request.targets;
    cmd.args(targets.as_slice());
    Ok(())
}

pub(super) fn configure_ninja_tool_command(
    cmd: &mut Command,
    request: &NinjaToolRequest<'_>,
) -> io::Result<()> {
    configure_ninja_base(cmd, request.options, request.build_file, request.env)?;
    cmd.arg("-t").arg(request.tool);
    Ok(())
}
