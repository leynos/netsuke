//! Localization audit for the build script.
//!
//! Compares the keys declared by `define_keys!` in `src/localization/keys.rs`
//! with every catalogue named by the locale registry in
//! `src/locale_catalogues.rs`. The audit fails the build when a catalogue is
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

use crate::locale_catalogues::{LocaleCatalogue, SOURCE_LOCALE, SUPPORTED_LOCALES};
use cap_std::{ambient_authority, fs::Dir};
use compare::{audit_catalogue, build_error_message};
use ftl::MessageVariables;
use metadata::parse_metadata_locales;
use std::path::{Path, PathBuf};
use std::{error::Error, io};

/// Repository-relative path of the source file that defines the localization keys.
const KEYS_PATH: &str = "src/localization/keys.rs";
/// Repository-relative path of the manifest whose locale metadata is audited.
const CARGO_MANIFEST: &str = "Cargo.toml";

/// Path of the catalogue for `tag`, relative to the repository root.
pub(crate) fn catalogue_path(tag: &str) -> PathBuf {
    catalogue_path_in(Path::new(""), tag)
}

/// Path of the catalogue for `tag`, beneath `root`.
fn catalogue_path_in(root: &Path, tag: &str) -> PathBuf {
    root.join("locales").join(tag).join("messages.ftl")
}

/// Verify that `Cargo.toml` advertises exactly the registry's locales.
///
/// The metadata is consumed by `ortho_config` tooling, so it must not drift
/// from the catalogues the binary actually embeds.
fn audit_cargo_metadata(root: &Path) -> Result<(), Box<dyn Error>> {
    let manifest = read_source(&root.join(CARGO_MANIFEST))?;
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

/// Read `path`, naming it in the error.
///
/// Every filesystem read the audit performs goes through a directory capability
/// here. The parsers below it take `&str`, so this module keeps the path-shaped
/// test seam while limiting each read to its parent directory.
fn read_source(path: &Path) -> Result<String, Box<dyn Error>> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("cannot read directory as source: {}", path.display()),
        )
    })?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())
        .map_err(|err| format!("failed to open {}: {err}", parent.display()))?;
    directory
        .read_to_string(name)
        .map_err(|err| format!("failed to read {}: {err}", path.display()).into())
}

/// Parse the catalogue at `path`, naming it if parsing fails.
///
/// The parser takes text and so cannot name the file itself; the path is added
/// back here, where it is known, so a failure still says which catalogue.
fn parse_catalogue_at(path: &Path) -> Result<MessageVariables, Box<dyn Error>> {
    ftl::parse_catalogue(&read_source(path)?)
        .map_err(|err| format!("{}: {err}", path.display()).into())
}

/// Parse the source-locale catalogue into message variables.
///
/// The source catalogue provides the variable declarations used when comparing
/// every other locale against the key registry.
fn source_catalogue_variables(root: &Path) -> Result<MessageVariables, Box<dyn Error>> {
    parse_catalogue_at(&catalogue_path_in(root, SOURCE_LOCALE))
}

/// Audit every registered locale, reporting the comparison findings together.
///
/// This is the audit's entry point, called from `build.rs`. It checks that
/// `Cargo.toml`'s locale metadata matches the registry, then compares each
/// catalogue against the keys `define_keys!` declares and against the English
/// source's interpolation variables.
///
/// The two kinds of failure surface differently. A catalogue that disagrees
/// with the source is a *finding*: every locale is compared and the findings
/// are reported in one message, so a translator sees the whole list rather
/// than fixing one key per build. Anything that stops the audit running at all
/// — drifted metadata, an unreadable file, an unparseable key macro — aborts
/// at the point it is met, because there is nothing further to compare.
///
/// # Errors
///
/// Returns an error when the metadata has drifted from the registry, when a
/// catalogue cannot be read, or when any catalogue is missing a declared key,
/// carries an orphaned key, or interpolates the wrong variables. The message
/// names every offending locale and key so one build reports them all.
pub(super) fn audit_localization_keys() -> Result<(), Box<dyn Error>> {
    audit_localization_keys_in(Path::new(""))
}

/// Audit the tree rooted at `root`.
///
/// Split from [`audit_localization_keys`] so tests can run the whole
/// orchestration against the checked-in repository without depending on the
/// process working directory. The build script keeps calling the entry point
/// above, which passes an empty root and so reads the same relative paths it
/// always did.
///
/// # Errors
///
/// As [`audit_localization_keys`].
pub(crate) fn audit_localization_keys_in(root: &Path) -> Result<(), Box<dyn Error>> {
    audit_cargo_metadata(root)?;
    let keys_path = root.join(KEYS_PATH);
    let declared = keys::extract_key_constants(&read_source(&keys_path)?)
        .map_err(|err| format!("{}: {err}", keys_path.display()))?;
    let source = source_catalogue_variables(root)?;

    let mut findings = Vec::new();
    for entry in SUPPORTED_LOCALES {
        let catalogue = parse_catalogue_at(&catalogue_path_in(root, entry.tag()))?;
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
