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

mod ftl;
mod keys;
mod metadata;

use crate::localization::locales::{LocaleCatalogue, SOURCE_LOCALE, SUPPORTED_LOCALES};
use ftl::MessageVariables;
use metadata::parse_metadata_locales;
use std::collections::BTreeSet;
use std::error::Error;
use std::path::{Path, PathBuf};

const KEYS_PATH: &str = "src/localization/keys.rs";
const CARGO_MANIFEST: &str = "Cargo.toml";

/// Findings for a single catalogue.
struct LocaleFindings {
    tag: &'static str,
    missing: Vec<String>,
    orphaned: Vec<String>,
    variable_mismatches: Vec<String>,
}

impl LocaleFindings {
    const fn is_clean(&self) -> bool {
        self.missing.is_empty() && self.orphaned.is_empty() && self.variable_mismatches.is_empty()
    }

    fn append_to(&self, message: &mut String) {
        append_section(message, self.tag, "missing", &self.missing);
        append_section(message, self.tag, "orphaned", &self.orphaned);
        append_section(
            message,
            self.tag,
            "variable mismatch",
            &self.variable_mismatches,
        );
    }
}

fn append_section(message: &mut String, tag: &str, label: &str, entries: &[String]) {
    if entries.is_empty() {
        return;
    }
    message.push_str("\n- ");
    message.push_str(label);
    message.push_str(" in ");
    message.push_str(tag);
    message.push_str(": ");
    message.push_str(&entries.join(", "));
}

/// Path of the catalogue for `tag`.
pub(crate) fn catalogue_path(tag: &str) -> PathBuf {
    Path::new("locales").join(tag).join("messages.ftl")
}

fn describe_variable_mismatch(
    key: &str,
    source: &BTreeSet<String>,
    other: &BTreeSet<String>,
) -> String {
    let render = |names: &BTreeSet<String>| {
        if names.is_empty() {
            "none".to_owned()
        } else {
            names
                .iter()
                .map(|name| format!("${name}"))
                .collect::<Vec<_>>()
                .join(" ")
        }
    };
    format!(
        "{key} (expected {}, found {})",
        render(source),
        render(other)
    )
}

fn variable_mismatches(source: &MessageVariables, other: &MessageVariables) -> Vec<String> {
    source
        .iter()
        .filter_map(|(key, expected)| {
            let found = other.get(key)?;
            (found != expected).then(|| describe_variable_mismatch(key, expected, found))
        })
        .collect()
}

fn audit_catalogue(
    tag: &'static str,
    declared: &BTreeSet<String>,
    source: &MessageVariables,
    catalogue: &MessageVariables,
) -> LocaleFindings {
    let present: BTreeSet<String> = catalogue.keys().cloned().collect();
    LocaleFindings {
        tag,
        missing: declared.difference(&present).cloned().collect(),
        orphaned: present.difference(declared).cloned().collect(),
        variable_mismatches: variable_mismatches(source, catalogue),
    }
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

fn build_error_message(findings: &[LocaleFindings]) -> String {
    let mut message = String::from("localization audit failed:");
    for finding in findings {
        finding.append_to(&mut message);
    }
    message
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
