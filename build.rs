//! Build script: generate the CLI manual page into target/generated-man for
//! release packaging.
use clap::CommandFactory;
use clap_mangen::Man;
use std::{env, fs, path::PathBuf};

#[path = "src/cli.rs"]
#[expect(
    dead_code,
    reason = "Only type definitions are needed for man page generation"
)]
mod cli;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Regenerate the manual page when the CLI or metadata changes.
    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_NAME");
    println!("cargo:rerun-if-env-changed=CARGO_BIN_NAME");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_DESCRIPTION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_AUTHORS");

    // Packagers expect man pages inside the crate directory under target/.
    let out_dir = PathBuf::from("target/generated-man");
    fs::create_dir_all(&out_dir)?;

    // The top-level page documents the entire command interface.
    let cmd = cli::Cli::command();
    let name = cmd.get_name().to_owned();
    let cargo_bin = env::var("CARGO_BIN_NAME")
        .or_else(|_| env::var("CARGO_PKG_NAME"))
        .unwrap_or_else(|_| name.clone());
    if name != cargo_bin {
        return Err(format!(
            "CLI name '{name}' differs from Cargo bin/package name '{cargo_bin}'; packaging expects {cargo_bin}.1"
        )
        .into());
    }
    let man = Man::new(cmd);
    let mut buf = Vec::new();
    man.render(&mut buf)?;
    let out_path = out_dir.join(format!("{cargo_bin}.1"));
    let tmp = out_dir.join(format!("{cargo_bin}.1.tmp"));
    fs::write(&tmp, &buf)?;
    if out_path.exists() {
        fs::remove_file(&out_path)?;
    }
    fs::rename(&tmp, &out_path)?;

    Ok(())
}
