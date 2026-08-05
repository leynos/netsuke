//! Build script for Netsuke.
//!
//! This script performs two main tasks:
//! - Generate the CLI manual page into `target/generated-man/<target>/<profile>` for release
//!   packaging.
//! - Audit localization keys declared in `src/localization/keys.rs` against the Fluent bundles
//!   in `locales/*/messages.ftl`, failing the build if any declared key is missing from a
//!   locale.
use clap::CommandFactory;
use clap_mangen::Man;
use std::{
    env, fs,
    path::{Path, PathBuf},
};
use time::{OffsetDateTime, format_description::well_known::Iso8601};

const FALLBACK_DATE: &str = "1970-01-01";

// The build script recompiles a slice of the library as its own crate so that
// `cli::Cli::command()` (used for man-page generation) can be constructed, and
// so that the localization audit can read the declared key registry.
//
// The slice is named file by file rather than by pulling in `src/cli/mod.rs`,
// because that would drag the whole `cli` subtree — configuration discovery,
// merging, diagnostics, localised value parsing — into this crate, where none
// of it is reachable. Recompiling only what is reachable keeps rustc's
// unused-item analysis meaningful here instead of requiring module-wide
// `#[expect(dead_code)]` suppressions that would also mask genuinely dead
// library code.
//
// The library modules below are laid out to keep this slice small:
// `src/cli/command.rs` holds definitions only, with runtime behaviour in
// `src/cli/preferences.rs` and `src/cli/parser.rs`; matching logic is split out
// of `src/host_pattern.rs` into `src/host_matching.rs`. Adding a dependency on
// anything outside this slice will surface here as a compile error, which is
// the intended signal.
#[path = "src/cli"]
mod cli {
    //! The Clap schema slice of `src/cli`, mirroring `src/cli/mod.rs`.

    #[path = "config.rs"]
    pub mod config;
    #[path = "validation.rs"]
    mod validation;

    #[path = "command.rs"]
    mod command;

    pub use command::Cli;
    pub use config::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
}

#[path = "src/cli_localization.rs"]
mod cli_localization;

#[path = "src/host_pattern.rs"]
mod host_pattern;

#[path = "src/localization/mod.rs"]
mod localization;

mod build_l10n_audit;

#[expect(
    clippy::disallowed_methods,
    reason = "SOURCE_DATE_EPOCH is the reproducible-builds contract: the build system supplies it to the build script's process, so there is no seam to inject it through"
)]
fn manual_date() -> String {
    let Ok(raw) = env::var("SOURCE_DATE_EPOCH") else {
        return FALLBACK_DATE.into();
    };

    let Ok(ts) = raw.parse::<i64>() else {
        println!(
            "cargo:warning=Invalid SOURCE_DATE_EPOCH '{raw}'; expected integer seconds since Unix epoch; falling back to {FALLBACK_DATE}"
        );
        return FALLBACK_DATE.into();
    };

    let Ok(dt) = OffsetDateTime::from_unix_timestamp(ts) else {
        println!(
            "cargo:warning=Invalid SOURCE_DATE_EPOCH '{raw}'; not a valid Unix timestamp; falling back to {FALLBACK_DATE}"
        );
        return FALLBACK_DATE.into();
    };

    dt.format(&Iso8601::DATE).unwrap_or_else(|_| {
        println!(
            "cargo:warning=Invalid SOURCE_DATE_EPOCH '{raw}'; formatting failed; falling back to {FALLBACK_DATE}"
        );
        FALLBACK_DATE.into()
    })
}

#[expect(
    clippy::disallowed_methods,
    reason = "TARGET and PROFILE are set by Cargo for the build script alone; nothing else knows the triple and profile being built, so they cannot be passed in"
)]
fn out_dir_for_target_profile() -> PathBuf {
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown-target".into());
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown-profile".into());
    PathBuf::from(format!("target/generated-man/{target}/{profile}"))
}

fn write_man_page(data: &[u8], dir: &Path, page_name: &str) -> std::io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let destination = dir.join(page_name);
    let tmp = dir.join(format!("{page_name}.tmp"));
    fs::write(&tmp, data)?;
    if destination.exists() {
        fs::remove_file(&destination)?;
    }
    fs::rename(&tmp, &destination)?;
    Ok(destination)
}

fn emit_rerun_directives() {
    // Only the modules this script actually compiles need to trigger a rerun.
    println!("cargo:rerun-if-changed=src/cli/command.rs");
    println!("cargo:rerun-if-changed=src/cli/config.rs");
    println!("cargo:rerun-if-changed=src/cli/validation.rs");
    println!("cargo:rerun-if-changed=src/host_pattern.rs");
    println!("cargo:rerun-if-changed=src/cli_localization.rs");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_NAME");
    println!("cargo:rerun-if-env-changed=CARGO_BIN_NAME");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_DESCRIPTION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_AUTHORS");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=PROFILE");
    println!("cargo:rerun-if-changed=src/localization/keys.rs");
    println!("cargo:rerun-if-changed=locales/en-US/messages.ftl");
    println!("cargo:rerun-if-changed=locales/es-ES/messages.ftl");
}

#[expect(
    clippy::disallowed_methods,
    reason = "CARGO_BIN_NAME, CARGO_PKG_NAME, CARGO_PKG_VERSION and OUT_DIR are Cargo's own build-script inputs; they describe the crate being compiled and Cargo provides them only through the environment"
)]
fn generate_man_page(out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
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
            "CLI name {name} differs from Cargo bin/package name {cargo_bin}; packaging expects {cargo_bin}.1"
        )
        .into());
    }
    let version = env::var("CARGO_PKG_VERSION").map_err(
        |_| "CARGO_PKG_VERSION must be set by Cargo; cannot render manual page without it.",
    )?;
    let man = Man::new(cmd)
        .section("1")
        .source(format!("{cargo_bin} {version}"))
        .date(manual_date());
    let mut buf = Vec::new();
    man.render(&mut buf)?;
    let page_name = format!("{cargo_bin}.1");
    write_man_page(&buf, out_dir, &page_name)?;
    if let Some(extra_dir) = env::var_os("OUT_DIR") {
        let extra_dir_path = PathBuf::from(extra_dir);
        if let Err(err) = write_man_page(&buf, &extra_dir_path, &page_name) {
            println!(
                "cargo:warning=Failed to stage manual page in OUT_DIR ({}): {err}",
                extra_dir_path.display()
            );
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    emit_rerun_directives();
    build_l10n_audit::audit_localization_keys()?;
    let out_dir = out_dir_for_target_profile();
    generate_man_page(&out_dir)
}
