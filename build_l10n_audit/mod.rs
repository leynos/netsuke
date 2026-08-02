//! Localization audit for the build script.
//!
//! Compares the keys declared by `define_keys!` in `src/localization/keys.rs`
//! with every catalogue named by the locale registry in
//! `src/localization/locales.rs`. The audit fails the build when a catalogue is
//! missing a declared key, carries an orphaned key, or interpolates a different
//! set of variables from the English source for a shared message.
//!
//! The registry is the sole locale list; `Cargo.toml`'s
//! `package.metadata.ortho_config.locales` array is checked against it rather
//! than being a second source of truth.

mod compare;
mod ftl;
mod keys;
mod metadata;

use crate::localization::locales::{LocaleCatalogue, SOURCE_LOCALE, SUPPORTED_LOCALES};
use compare::{audit_catalogue, build_error_message};
use ftl::MessageVariables;
use metadata::parse_metadata_locales;
use std::error::Error;
use std::path::{Path, PathBuf};

const KEYS_PATH: &str = "src/localization/keys.rs";
const CARGO_MANIFEST: &str = "Cargo.toml";

/// Path of the catalogue for `tag`.
pub(crate) fn catalogue_path(tag: &str) -> PathBuf {
    Path::new("locales").join(tag).join("messages.ftl")
}

/// Verify that `Cargo.toml` advertises exactly the registry's locales.
///
/// The metadata is consumed by `ortho_config` tooling, so it must not drift
/// from the catalogues the binary actually embeds.
fn audit_cargo_metadata() -> Result<(), Box<dyn Error>> {
    let manifest = std::fs::read_to_string(CARGO_MANIFEST)
        .map_err(|err| format!("failed to read {CARGO_MANIFEST}: {err}"))?;
    let declared = parse_metadata_locales(&manifest)
        .ok_or("Cargo.toml is missing package.metadata.ortho_config.locales")?;
    let expected: Vec<&str> = SUPPORTED_LOCALES.iter().map(LocaleCatalogue::tag).collect();
    if declared == expected {
        return Ok(());
    }
    Err(format!(
        "Cargo.toml package.metadata.ortho_config.locales does not match the locale registry:\n\
         - Cargo.toml: {}\n- registry:   {}",
        declared.join(", "),
        expected.join(", ")
    )
    .into())
}

fn source_catalogue_variables() -> Result<MessageVariables, Box<dyn Error>> {
    ftl::parse_catalogue(&catalogue_path(SOURCE_LOCALE))
}

/// Audit every registered locale, failing the build on the first problem.
///
/// This is the audit's entry point, called from `build.rs`. It checks that
/// `Cargo.toml`'s locale metadata matches the registry, then compares each
/// catalogue against the keys `define_keys!` declares and against the English
/// source's interpolation variables.
///
/// # Errors
///
/// Returns an error when the metadata has drifted from the registry, when a
/// catalogue cannot be read, or when any catalogue is missing a declared key,
/// carries an orphaned key, or interpolates the wrong variables. The message
/// names every offending locale and key so one build reports them all.
pub(super) fn audit_localization_keys() -> Result<(), Box<dyn Error>> {
    audit_cargo_metadata()?;
    let declared = keys::extract_key_constants(Path::new(KEYS_PATH))?;
    let source = source_catalogue_variables()?;

    let mut findings = Vec::new();
    for entry in SUPPORTED_LOCALES {
        let catalogue = ftl::parse_catalogue(&catalogue_path(entry.tag()))?;
        let result = audit_catalogue(entry.tag(), &declared, &source, &catalogue);
        if !result.is_clean() {
            findings.push(result);
        }
    }

    if findings.is_empty() {
        Ok(())
    } else {
        Err(build_error_message(&findings).into())
    }
}
