//! Build script for Netsuke.
//!
//! This script performs three main tasks:
//! - Generate the CLI manual page into `target/generated-man/<target>/<profile>` for release
//!   packaging.
//! - Generate Bash, Elvish, Fish, PowerShell, and Zsh completion files into
//!   `target/generated-completions/<target>/<profile>` from the same Clap command tree.
//! - Audit localization keys declared in `src/localization/keys.rs` against the Fluent bundles
//!   in `locales/*/messages.ftl`, failing the build if any declared key is missing from a
//!   locale.
use cap_std::{ambient_authority, fs::Dir};
use clap_complete::aot::{Shell, generate_to};
use clap_mangen::Man;
use std::{
    env,
    path::{Path, PathBuf},
};
use time::{OffsetDateTime, format_description::well_known::Iso8601};

/// Calendar date used when `SOURCE_DATE_EPOCH` is unset or invalid.
///
/// Keeping a fixed fallback makes builds reproducible when the build system
/// does not supply the reproducible-builds epoch.
const FALLBACK_DATE: &str = "1970-01-01";

// The build script recompiles the parser subset needed to construct
// `cli::Cli::command()` for man-page generation. Runtime discovery is excluded:
// the build script does not perform discovery, and compiling it here would pull
// its ambient canonicalization boundary into this separate compilation unit.
// The parser subset exposes more library API than this binary reaches, so the
// compiler reports unused items that the library crate and its tests exercise.
#[expect(
    dead_code,
    unused_imports,
    reason = "shared library source; the unreached API is exercised by the library crate"
)]
#[path = "src/cli/build_support.rs"]
mod cli;

#[path = "src/cli_localization.rs"]
mod cli_localization;

#[expect(
    dead_code,
    reason = "shared library source; the unreached API is exercised by the library crate"
)]
#[path = "src/cli_l10n.rs"]
mod cli_l10n;

#[expect(
    dead_code,
    reason = "shared library source; the unreached API is exercised by the library crate"
)]
#[path = "src/host_pattern.rs"]
mod host_pattern;

/// The locale registry, shared with the library crate.
///
/// Both `localization` and the audit reach the registry through
/// `crate::locale_catalogues`, so it is declared at this crate's root under
/// that name. It is public because `localization` re-exports it, and a private
/// module cannot be re-exported from a public path. The build script itself
/// reads `SUPPORTED_LOCALES` to emit one `rerun-if-changed` directive per
/// catalogue.
#[path = "src/locale_catalogues.rs"]
pub mod locale_catalogues;

/// Message rendering, shared with the library crate.
///
/// Exposed as `crate::localization`, which `cli`, `cli_l10n`, and
/// `host_pattern` reach for `localization::keys` when building the clap
/// command for man-page generation. Public so its `locales` re-export stays
/// reachable at `crate::localization::locales`.
#[path = "src/localization/mod.rs"]
pub mod localization;

#[expect(
    dead_code,
    reason = "shared library source; the unreached API is exercised by the library crate"
)]
#[path = "src/output_mode.rs"]
mod output_mode;

#[expect(
    dead_code,
    reason = "shared library source; the unreached API is exercised by the library crate"
)]
#[path = "src/theme.rs"]
mod theme;

mod build_l10n_audit;

/// Compute the manual page date from `SOURCE_DATE_EPOCH`.
///
/// Returns the epoch parsed as a Unix timestamp formatted as an ISO 8601 date;
/// when the variable is unset or malformed the [`FALLBACK_DATE`] is returned so
/// the generated page stays deterministic.
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

/// Return the generated-artefact directory for this build's target triple and profile.
///
/// Uses the `target/{artefact}/{target}/{profile}` layout so man pages and
/// completions never mix between artefacts, host builds, and cross builds.
#[expect(
    clippy::disallowed_methods,
    reason = "TARGET and PROFILE are set by Cargo for the build script alone; nothing else knows the triple and profile being built, so they cannot be passed in"
)]
fn out_dir_for_target_profile(artefact: &str) -> PathBuf {
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown-target".into());
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown-profile".into());
    PathBuf::from(format!("target/{artefact}/{target}/{profile}"))
}

/// Write one man page into `dir` atomically and return its path.
///
/// The bytes are written to a temporary name inside `dir` and then renamed over
/// the final name, so a failed write never leaves a partial page behind.
///
/// # Errors
///
/// Returns an I/O error when the destination directory cannot be opened or
/// created, the temporary file cannot be written, or the atomic rename fails.
fn write_man_page(data: &[u8], dir: &Path, page_name: &str) -> std::io::Result<PathBuf> {
    let man_dir = if dir.is_relative() {
        let working_dir = Dir::open_ambient_dir(".", ambient_authority())?;
        working_dir.create_dir_all(dir)?;
        working_dir.open_dir(dir)?
    } else {
        // Cargo creates `OUT_DIR` before the build script runs, so this boundary
        // can narrow the build process's ambient authority to that directory.
        Dir::open_ambient_dir(dir, ambient_authority())?
    };
    let temporary_name = format!("{page_name}.tmp");
    man_dir.write(&temporary_name, data)?;
    man_dir.rename(&temporary_name, &man_dir, page_name)?;
    Ok(dir.join(page_name))
}

/// Emit Cargo's `rerun-if-changed` directives for the generated sources.
///
/// Changing any listed file must rerun the build script so the man page and
/// completions stay in sync with the CLI they describe.
fn emit_rerun_directives() {
    println!("cargo:rerun-if-changed=src/cli/build_support.rs");
    println!("cargo:rerun-if-changed=src/cli/config.rs");
    println!("cargo:rerun-if-changed=src/cli/help.rs");
    println!("cargo:rerun-if-changed=src/cli/parser.rs");
    println!("cargo:rerun-if-changed=src/cli/parsing.rs");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_DESCRIPTION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_AUTHORS");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=PROFILE");
    println!("cargo:rerun-if-changed=src/localization/keys.rs");
    println!("cargo:rerun-if-changed=src/locale_catalogues.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    // The locale registry owns the catalogue list, so the rerun directives are
    // derived from it rather than repeated by hand.
    for entry in locale_catalogues::SUPPORTED_LOCALES {
        println!(
            "cargo:rerun-if-changed={}",
            build_l10n_audit::catalogue_path(entry.tag()).display()
        );
    }
}

/// Generate and install the CLI manual page under `out_dir`.
///
/// The page is rendered from the same Clap command tree used at runtime, so the
/// installed documentation cannot drift from the actual CLI.
///
/// # Errors
///
/// Returns an error when Cargo's package-version environment is missing, when
/// rendering the command tree fails, or when the generated page cannot be
/// written to its destination.
#[expect(
    clippy::disallowed_methods,
    reason = "CARGO_PKG_VERSION and OUT_DIR are Cargo's own build-script inputs; they describe the crate being compiled and Cargo provides them only through the environment"
)]
fn generate_man_page(out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Build artefacts preserve the source en-US wording while still using the
    // configured parser metadata, so documentation stays deterministic.
    let cmd = cli::configured_command(None);
    let name = cmd
        .get_bin_name()
        .unwrap_or_else(|| cmd.get_name())
        .to_owned();
    let version = env::var("CARGO_PKG_VERSION").map_err(
        |_| "CARGO_PKG_VERSION must be set by Cargo; cannot render manual page without it.",
    )?;
    let man = Man::new(cmd)
        .section("1")
        .source(format!("{name} {version}"))
        .date(manual_date());
    let mut buf = Vec::new();
    man.render(&mut buf)?;
    let page_name = format!("{name}.1");
    let destination = write_man_page(&buf, out_dir, &page_name)?;
    // Publish the destination so the crate's tests can assert the manual page
    // contract (name, location, and `.TH` source) without re-deriving where the
    // build script chose to write it.
    println!(
        "cargo:rustc-env=NETSUKE_GENERATED_MAN_PAGE={}",
        destination.display()
    );
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

/// Generate shell-completion files under `out_dir` for every supported shell.
///
/// Completions are derived from the same Clap command tree as the man page, so
/// they cannot drift from the shipped CLI surface.
///
/// # Errors
///
/// Returns an error when the output directory cannot be opened or created, or
/// when completion generation fails for a supported shell.
fn generate_completions(out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let working_dir = Dir::open_ambient_dir(".", ambient_authority())?;
    working_dir.create_dir_all(out_dir)?;
    // Keep completion metadata in the same source en-US wording as the manual.
    let cli_command = cli::configured_command(None);
    let name = cli_command
        .get_bin_name()
        .unwrap_or_else(|| cli_command.get_name())
        .to_owned();

    for shell in [
        Shell::Bash,
        Shell::Elvish,
        Shell::Fish,
        Shell::PowerShell,
        Shell::Zsh,
    ] {
        let mut completion_command = cli::configured_command(None);
        generate_to(shell, &mut completion_command, &name, out_dir)?;
    }

    // Publish the directory so tests can inspect the exact generated artefacts
    // rather than recreating the generator's path and file-name conventions.
    println!(
        "cargo:rustc-env=NETSUKE_GENERATED_COMPLETIONS_DIR={}",
        out_dir.display()
    );
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    emit_rerun_directives();
    build_l10n_audit::audit_localization_keys()?;
    generate_man_page(&out_dir_for_target_profile("generated-man"))?;
    generate_completions(&out_dir_for_target_profile("generated-completions"))
}
