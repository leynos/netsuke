//! End-to-end tests for the build-time localization audit.
//!
//! The unit tests in `build_l10n_parser_tests.rs` cover the audit's rules
//! against synthetic input. This file runs the real orchestration over the
//! checked-in tree — the registry, `Cargo.toml`'s metadata, the declared keys,
//! and all 35 catalogues — so that drift in any of them fails the test suite
//! rather than only the next clean build.
//!
//! The audit lives in a build script, which is not a test target, so its
//! modules are included by path. `locale_catalogues` comes with them because
//! the audit reads the registry through `crate::locale_catalogues`.

use anyhow::{Result, bail, ensure};
use rstest::rstest;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// These modules are production source included by path. This crate reaches
// only the audit entry point and the registry's tag list, so the rest of each
// module's surface is unused *here*; it is exercised where it actually lives,
// by the library crate and the build script, whose own dead-code analysis is
// untouched by this expectation.
#[expect(
    dead_code,
    reason = "production source included by path; only the audit entry point is reachable here"
)]
#[path = "../src/locale_catalogues.rs"]
mod locale_catalogues;

#[expect(
    dead_code,
    reason = "production source included by path; only the audit entry point is reachable here"
)]
#[path = "../build_l10n_audit/mod.rs"]
mod build_l10n_audit;

use build_l10n_audit::audit_localization_keys_in;
use locale_catalogues::{LocaleCatalogue, SUPPORTED_LOCALES};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The audit must pass against the tree as committed.
///
/// This is the test that fails when a catalogue loses a key, gains an orphan,
/// or changes an interpolation variable, and when `Cargo.toml`'s locale list
/// drifts from the registry.
#[test]
fn the_audit_passes_over_the_checked_in_tree() -> Result<()> {
    if let Err(error) = audit_localization_keys_in(&repository_root()) {
        bail!("the committed tree must pass its own localization audit: {error}");
    }
    Ok(())
}

/// Copy the repository's audit inputs into `destination`.
///
/// Only the files the audit reads are staged, so a mutation test perturbs a
/// copy and never the working tree.
fn stage_audit_inputs(destination: &Path) -> Result<()> {
    let root = repository_root();
    fs::create_dir_all(destination.join("src/localization"))?;
    fs::copy(root.join("Cargo.toml"), destination.join("Cargo.toml"))?;
    fs::copy(
        root.join("src/localization/keys.rs"),
        destination.join("src/localization/keys.rs"),
    )?;
    for entry in SUPPORTED_LOCALES {
        let relative = Path::new("locales").join(entry.tag());
        fs::create_dir_all(destination.join(&relative))?;
        fs::copy(
            root.join(&relative).join("messages.ftl"),
            destination.join(&relative).join("messages.ftl"),
        )?;
    }
    Ok(())
}

/// A staged copy of the audit inputs, ready to be perturbed.
fn staged_tree() -> Result<TempDir> {
    let staged = TempDir::new()?;
    stage_audit_inputs(staged.path())?;
    Ok(staged)
}

/// The staged copy must itself pass, or the mutation cases below would prove
/// nothing: a failure could mean the staging was incomplete rather than that
/// the mutation was detected.
#[test]
fn the_staged_copy_reproduces_a_passing_audit() -> Result<()> {
    let staged = staged_tree()?;
    if let Err(error) = audit_localization_keys_in(staged.path()) {
        bail!("staged inputs must reproduce a passing audit: {error}");
    }
    Ok(())
}

/// Each way the tree can drift must fail the audit.
///
/// Every case perturbs one input and asserts the audit rejects it, which is
/// what makes the passing test above meaningful rather than vacuous.
#[rstest]
#[case::a_catalogue_loses_a_key("missing in fr", &|root: &Path| {
    let catalogue = root.join("locales/fr/messages.ftl");
    let text = fs::read_to_string(&catalogue)?;
    let kept: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with("cli.about"))
        .collect();
    ensure!(kept.len() < text.lines().count(), "expected a cli.about message to drop");
    fs::write(&catalogue, kept.join("\n"))?;
    Ok(())
})]
#[case::a_catalogue_gains_an_orphan("orphaned in de", &|root: &Path| {
    let catalogue = root.join("locales/de/messages.ftl");
    let mut text = fs::read_to_string(&catalogue)?;
    text.push_str("\nnetsuke.not.a.declared.key = Verwaist\n");
    fs::write(&catalogue, text)?;
    Ok(())
})]
#[case::a_catalogue_changes_a_variable("variable mismatch in it", &|root: &Path| {
    let catalogue = root.join("locales/it/messages.ftl");
    let text = fs::read_to_string(&catalogue)?;
    let mutated = text.replacen("{ $path }", "{ $percorso }", 1);
    ensure!(mutated != text, "expected a $path interpolation to rewrite");
    fs::write(&catalogue, mutated)?;
    Ok(())
})]
#[case::the_metadata_drops_a_locale("does not match the locale registry", &|root: &Path| {
    let manifest = root.join("Cargo.toml");
    let text = fs::read_to_string(&manifest)?;
    let mutated = text.replacen("\"zh-Hant\",", "", 1).replacen("\"zh-Hant\"", "", 1);
    ensure!(mutated != text, "expected zh-Hant in the metadata list");
    fs::write(&manifest, mutated)?;
    Ok(())
})]
#[case::a_key_declaration_is_added("missing in", &|root: &Path| {
    let keys = root.join("src/localization/keys.rs");
    let text = fs::read_to_string(&keys)?;
    let mutated = text.replacen(
        "define_keys! {",
        "define_keys! {\n    UNSHIPPED_KEY => \"netsuke.unshipped.key\",",
        1,
    );
    ensure!(mutated != text, "expected the define_keys! macro to rewrite");
    fs::write(&keys, mutated)?;
    Ok(())
})]
fn the_audit_rejects_a_drifting_tree(
    #[case] expected: &str,
    #[case] mutate: &dyn Fn(&Path) -> Result<()>,
) -> Result<()> {
    let staged = staged_tree()?;
    mutate(staged.path())?;

    let message = match audit_localization_keys_in(staged.path()) {
        Ok(()) => bail!("the audit accepted a tree it should have rejected"),
        Err(error) => error.to_string(),
    };
    ensure!(
        message.contains(expected),
        "expected the failure to mention {expected:?}, got {message}"
    );
    Ok(())
}

/// Every registry entry must have a catalogue the audit can actually read.
///
/// `include_str!` already fails the build for a missing file, but it embeds at
/// compile time; this checks the on-disk path the audit reads at build time
/// resolves for the same set of tags.
#[test]
fn every_registry_tag_has_a_readable_catalogue() -> Result<()> {
    let root = repository_root();
    for entry in SUPPORTED_LOCALES {
        let path = root.join("locales").join(entry.tag()).join("messages.ftl");
        ensure!(
            path.is_file(),
            "locale {} has no catalogue at {}",
            entry.tag(),
            path.display()
        );
    }
    ensure!(
        SUPPORTED_LOCALES
            .iter()
            .map(LocaleCatalogue::tag)
            .any(|tag| tag == "en-US"),
        "the registry must contain the source locale"
    );
    Ok(())
}
