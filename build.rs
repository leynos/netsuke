use clap::CommandFactory;
use clap_mangen::Man;
use std::{fs, path::PathBuf};

#[path = "src/cli.rs"]
#[allow(
    dead_code,
    reason = "Only type definitions are needed for man page generation"
)]
mod cli;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Regenerate the manual page when the CLI changes.
    println!("cargo:rerun-if-changed=src/cli.rs");

    // Packagers expect man pages inside the crate directory under target/.
    let out_dir = PathBuf::from("target/generated-man");
    fs::create_dir_all(&out_dir)?;

    // The top-level page documents the entire command interface.
    let cmd = cli::Cli::command();
    let man = Man::new(cmd);
    let mut buf = Vec::new();
    man.render(&mut buf)?;
    fs::write(out_dir.join("netsuke.1"), buf)?;

    Ok(())
}
