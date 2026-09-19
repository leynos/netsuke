//! What the repository pins, read from the files that declare it.
//!
//! Split from the parent module to keep `sandbox` within the Whitaker
//! `module_max_lines` cap, and separated along the seam that was already
//! there: everything here reads a committed file and returns what it says,
//! where the parent builds and drives a sandbox. These functions take no
//! `Sandbox` and touch nothing it created, so a test that only wants a
//! pinned value — the standard's flags, the toolchain, the Cargo
//! configuration — never has to construct one.
//!
//! Reading the pins rather than restating them is what keeps a test agreeing
//! with the repository: a hard-coded flag would keep asserting the value it
//! was written against on the day the file changed.

use anyhow::{Context, Result};

use crate::fs;

/// Read a `tools/` version pin, so tests agree with the repository rather than
/// hard-coding a value that drifts on the next bump.
fn read_pin(path: &str) -> Result<String> {
    Ok(fs::read_to_string(path)
        .with_context(|| format!("read {path}"))?
        .trim()
        .to_owned())
}

/// The committed Cargo configuration's path, relative to the repository root.
///
/// Cargo auto-discovers this file, which is what makes the build standard the
/// default rather than something a target has to opt into.
pub const CARGO_CONFIG_PATH: &str = ".cargo/config.toml";

/// The committed Cargo configuration's contents, so a test can assert on what
/// every build actually applies rather than only on a recipe's command line.
///
/// # Errors
///
/// Returns an error if the checked-in Cargo configuration cannot be read.
pub fn cargo_config() -> Result<String> {
    fs::read_to_string(CARGO_CONFIG_PATH).with_context(|| format!("read {CARGO_CONFIG_PATH}"))
}

/// The flags the build standard applies on the platform this build runs on.
///
/// Read from the committed configuration rather than restated, which is what
/// keeps a recipe's composition and the file from drifting apart: every flag
/// the Makefile names must be one the configuration also names, or a bare
/// `cargo build` would not get it.
///
/// The two platforms answer from different sources. The build standard is
/// gated behind `cfg(target_os = "linux")` because `mold` ships for Linux
/// only, and Cargo replaces rather than merges two `rustflags` sources, so the
/// Linux table is the whole answer there and `[build]` is the whole answer
/// elsewhere. Reading one and comparing it to both would report the other
/// platform's flags as missing.
///
/// # Errors
///
/// Returns an error if the configuration cannot be read, cannot be parsed, or
/// carries no `rustflags` list for this platform.
pub fn standard_flags() -> Result<Vec<String>> {
    let config: toml::Value = toml::from_str(&cargo_config()?)?;
    let table = if cfg!(target_os = "linux") {
        config
            .get("target")
            .and_then(|value| value.get(r#"cfg(target_os = "linux")"#))
    } else {
        config.get("build")
    };
    let flags = table
        .and_then(|value| value.get("rustflags"))
        .and_then(toml::Value::as_array)
        .context("the configuration should carry a rustflags list for this platform")?;
    Ok(flags
        .iter()
        .filter_map(toml::Value::as_str)
        .map(str::to_owned)
        .collect())
}

/// The repository's pinned mold release tag.
///
/// # Errors
///
/// Returns an error if the pinned Mold version cannot be read.
pub fn pinned_mold_version() -> Result<String> {
    read_pin("tools/mold/VERSION")
}

/// The repository's toolchain, read from `rust-toolchain.toml`.
///
/// The build standard deliberately shares it rather than pinning a second
/// nightly, so the accelerated loop and the gates borrow-check identically;
/// the pinned nightly is what enables Polonius.
///
/// # Errors
///
/// Returns an error if the pinned Rust toolchain cannot be read.
pub fn pinned_toolchain() -> Result<String> {
    let contents = read_pin("rust-toolchain.toml")?;
    contents
        .lines()
        .find_map(|line| {
            let rest = line.trim().strip_prefix("channel")?;
            let value = rest.trim_start().strip_prefix('=')?;
            Some(value.trim().trim_matches('"').to_owned())
        })
        .context("rust-toolchain.toml should declare a channel")
}
