//! Build script: generate the CLI manual page into target/generated-man/<target>/<profile> for
//! release packaging.
use clap::CommandFactory;
use clap_mangen::Man;
use std::{env, fs, path::PathBuf};
use time::{OffsetDateTime, format_description::well_known::Iso8601};

const FALLBACK_DATE: &str = "1970-01-01";

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
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=PROFILE");

    // Packagers expect man pages under target/generated-man/<target>/<profile>.
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown-target".into());
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown-profile".into());
    let out_dir = PathBuf::from(format!("target/generated-man/{target}/{profile}"));
    fs::create_dir_all(&out_dir)?;

    // The top-level page documents the entire command interface.
    let cmd = cli::Cli::command();
    let name = cmd
        .get_bin_name()
        .unwrap_or_else(|| cmd.get_name())
        .to_owned();
    let cargo_bin = env::var("CARGO_BIN_NAME")
        .or_else(|_| env::var("CARGO_PKG_NAME"))
        .unwrap_or_else(|_| name.clone());
    if name != cargo_bin {
        return Err(format!(
            "CLI name '{name}' differs from Cargo bin/package name '{cargo_bin}'; packaging expects {cargo_bin}.1"
        )
        .into());
    }
    let version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION must be set");
    let date = env::var("SOURCE_DATE_EPOCH").map_or_else(
        |_| FALLBACK_DATE.into(),
        |raw| {
            raw.parse::<i64>()
                .ok()
                .and_then(|ts| OffsetDateTime::from_unix_timestamp(ts).ok())
                .and_then(|dt| dt.format(&Iso8601::DATE).ok())
                .unwrap_or_else(|| {
                    println!("cargo:warning=Invalid SOURCE_DATE_EPOCH '{raw}'; expected integer seconds since Unix epoch. Falling back to {FALLBACK_DATE}");
                    FALLBACK_DATE.into()
                })
        },
    );
    let man = Man::new(cmd)
        .section("1")
        .source(format!("{cargo_bin} {version}"))
        .date(date);
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
