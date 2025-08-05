//! CLI execution and command dispatch logic.
//!
//! This module keeps [`main`] minimal by providing a single entry point that
//! handles command execution. It now delegates build requests to the Ninja
//! subprocess, streaming its output back to the user.

use crate::{
    cli::{Cli, Commands},
    ir, manifest, ninja_gen,
};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use tempfile::NamedTempFile;

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
/// This loads the `Netsukefile`, converts it to an intermediate representation,
/// generates a temporary Ninja build script, and executes Ninja with this
/// script. Job count and working directory options are forwarded to Ninja, and
/// the child's standard output and error are streamed back to the user.
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
    // Keep the file handle alive so the temporary script outlives the child
    // process.
    let build_file = manifest_to_build_file(&cli.file)?;

    let mut cmd = Command::new(program);
    if let Some(dir) = &cli.directory {
        cmd.current_dir(dir).arg("-C").arg(dir);
    }
    if let Some(jobs) = cli.jobs {
        cmd.arg("-j").arg(jobs.to_string());
    }
    cmd.arg("-f").arg(build_file.path());
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
        #[expect(
            clippy::io_other_error,
            reason = "use explicit error kind for compatibility with older Rust"
        )]
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("ninja exited with {status}"),
        ))
    }
}

/// Generate a temporary Ninja build file from a manifest.
fn manifest_to_build_file(path: &Path) -> io::Result<NamedTempFile> {
    let manifest = manifest::from_path(path).map_err(io::Error::other)?;
    let graph = ir::BuildGraph::from_manifest(&manifest).map_err(io::Error::other)?;
    let ninja_script = ninja_gen::generate(&graph);

    let mut build_file = NamedTempFile::new().map_err(io::Error::other)?;
    build_file.write_all(ninja_script.as_bytes())?;
    build_file.flush()?;
    Ok(build_file)
}
