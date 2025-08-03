//! CLI execution and command dispatch logic.
//!
//! This module keeps [`main`] minimal by providing a single entry point that
//! handles command execution. It now delegates build requests to the Ninja
//! subprocess, streaming its output back to the user.

use crate::cli::{Cli, Commands};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

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
///
/// # Panics
///
/// Panics if the child's output streams cannot be captured.
pub fn run_ninja(program: &Path, cli: &Cli, targets: &[String]) -> io::Result<()> {
    let mut cmd = Command::new(program);
    if let Some(dir) = &cli.directory {
        cmd.current_dir(dir).arg("-C").arg(dir);
    }
    if let Some(jobs) = cli.jobs {
        cmd.arg("-j").arg(jobs.to_string());
    }
    cmd.args(targets);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().expect("child stdout");
    let stderr = child.stderr.take().expect("child stderr");

    let out_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut handle = io::stdout();
        for line in reader.lines().map_while(Result::ok) {
            let _ = writeln!(handle, "{line}");
        }
    });
    let err_handle = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut handle = io::stderr();
        for line in reader.lines().map_while(Result::ok) {
            let _ = writeln!(handle, "{line}");
        }
    });

    let status = child.wait()?;
    let _ = out_handle.join();
    let _ = err_handle.join();

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("ninja exited with {status}")))
    }
}
