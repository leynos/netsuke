//! CLI execution and command dispatch logic.
//!
//! This module keeps [`main`] minimal by providing a single entry point that
//! handles command execution. It now delegates build requests to the Ninja
//! subprocess, streaming its output back to the user.

use crate::cli::{Cli, Commands};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

/// Execute the parsed [`Cli`] commands.
///
/// # Errors
///
/// Returns an [`io::Error`] if the Ninja process fails to spawn or exits with a
/// non-zero status code.
pub fn run(cli: &Cli) -> io::Result<()> {
    let command = cli.command.clone().unwrap_or(Commands::Build {
        targets: Vec::new(),
    });
    match command {
        Commands::Build { targets } => run_ninja(Path::new("ninja"), cli, &targets),
        Commands::Clean => {
            println!("Clean requested");
            Ok(())
        }
        Commands::Graph => {
            println!("Graph requested");
            Ok(())
        }
    }
}

/// Invoke the Ninja executable with the provided CLI settings.
///
/// The function forwards the job count and working directory to Ninja and
/// streams its standard output and error back to the user.
///
/// # Errors
///
/// Returns an [`io::Error`] if the Ninja process fails to spawn or reports a
/// non-zero exit status.
pub fn run_ninja(program: &Path, cli: &Cli, targets: &[String]) -> io::Result<()> {
    let mut cmd = Command::new(program);
    if let Some(dir) = &cli.directory {
        cmd.current_dir(dir).arg("-C").arg(dir);
    }
    if let Some(jobs) = cli.jobs {
        cmd.arg("-j").arg(jobs.to_string());
    }
    cmd.args(targets);

    let output = cmd.output()?;
    io::stdout().write_all(&output.stdout)?;
    io::stderr().write_all(&output.stderr)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "ninja exited with {}",
            output.status
        )))
    }
}
