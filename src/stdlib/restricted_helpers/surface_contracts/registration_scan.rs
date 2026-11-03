//! The inventory checked against the registration sites in the source tree.
//!
//! Split from the single-file contracts module, which outgrew the module line
//! cap, along the claim it serves: completeness in both directions is a claim
//! about the source text rather than about a loaded manifest, so it needs no
//! probe surface and belongs beside the scanner that reads the registrations.

use anyhow::{Context as _, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

use super::super::{HelperKind, SURFACE, is_loader_owned};

/// Return every literal `add_function`/`add_filter`/`add_test` registration.
///
/// The scanner walks the source text rather than the compiled environment
/// because `MiniJinja` exposes no way to enumerate filters or tests, so the
/// registrations themselves are the only complete witness. Whitespace between
/// the call, the parenthesis, and the name literal is tolerated so a reformat
/// cannot silently hide a name.
///
/// The registration marker decides the helper kind. Functions, filters, and
/// tests occupy separate `MiniJinja` namespaces, so a name alone does not
/// identify a registration: a filter called `now` is a different helper from
/// the inventoried function `now`.
fn scanned_registrations() -> Result<Vec<(Utf8PathBuf, HelperKind, String)>> {
    const MARKERS: [(&str, HelperKind); 3] = [
        ("add_function", HelperKind::Function),
        ("add_filter", HelperKind::Filter),
        ("add_test", HelperKind::Test),
    ];
    let root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open the crate root")?;
    let mut sources = Vec::new();
    collect_sources(&root, Utf8Path::new("src/stdlib"), &mut sources)?;
    let mut found = Vec::new();
    for path in sources {
        let text = root
            .read_to_string(path.as_str())
            .with_context(|| format!("read {path}"))?;
        for (kind, name) in literal_names(&text, &MARKERS) {
            found.push((path.clone(), kind, name));
        }
    }
    Ok(found)
}

/// Append every Rust source beneath `directory` to `sources`.
fn collect_sources(root: &Dir, directory: &Utf8Path, sources: &mut Vec<Utf8PathBuf>) -> Result<()> {
    for entry in root
        .read_dir(directory)
        .with_context(|| format!("read `{directory}`"))?
    {
        let resolved = entry.with_context(|| format!("read an entry of `{directory}`"))?;
        let name = resolved
            .file_name()
            .with_context(|| format!("read an entry name in `{directory}`"))?;
        let path = directory.join(&name);
        if resolved.file_type()?.is_dir() {
            collect_sources(root, &path, sources)?;
        } else if path.extension() == Some("rs") {
            sources.push(path);
        }
    }
    Ok(())
}

/// Return every `marker("name"` occurrence in `text` with its helper kind.
#[expect(
    clippy::string_slice,
    reason = "every offset comes from `str::find`, so each one is a character boundary"
)]
fn literal_names(text: &str, markers: &[(&str, HelperKind)]) -> Vec<(HelperKind, String)> {
    let mut names = Vec::new();
    for (marker, kind) in markers {
        let mut from = 0;
        while let Some(offset) = text[from..].find(marker) {
            let after = from + offset + marker.len();
            if let Some(name) = literal_after(&text[after..]) {
                names.push((*kind, name));
            }
            from = after;
        }
    }
    names
}

/// Return the string literal following `(` and whitespace, if one is there.
#[expect(
    clippy::string_slice,
    reason = "the end offset comes from `str::find`, so it is a character boundary"
)]
fn literal_after(rest: &str) -> Option<String> {
    let after_paren = rest.trim_start().strip_prefix('(')?.trim_start();
    let quoted = after_paren.strip_prefix('"')?;
    let end = quoted.find('"')?;
    Some(quoted[..end].to_owned())
}

/// Report whether `SURFACE` inventories a registration of this kind and name.
fn is_inventoried(kind: HelperKind, name: &str) -> bool {
    SURFACE
        .iter()
        .any(|entry| entry.kind == kind && entry.name == name)
}

/// Assert every literal registration appears in `SURFACE`.
#[test]
fn every_scanned_registration_is_inventoried() -> Result<()> {
    let scanned = scanned_registrations()?;
    ensure!(
        !scanned.is_empty(),
        "the source scan found no registrations; the walk is broken"
    );
    let mut unexpected: Vec<String> = scanned
        .iter()
        .filter(|(_, kind, name)| !is_inventoried(*kind, name))
        .map(|(path, kind, name)| format!("`{kind:?} {name}` in {path}"))
        .collect();
    unexpected.sort();
    ensure!(
        unexpected.is_empty(),
        "literal registrations missing from `SURFACE`: {unexpected:?}"
    );
    Ok(())
}

/// Assert every invented name has a registration site or a stated exemption.
#[test]
fn every_inventory_name_is_registered_somewhere() -> Result<()> {
    let scanned = scanned_registrations()?;
    let mut missing: Vec<String> = SURFACE
        .iter()
        .filter(|entry| {
            let is_loop_registered = entry.kind == HelperKind::Test;
            let is_scanned = scanned
                .iter()
                .any(|(_, kind, name)| *kind == entry.kind && name == entry.name);
            !(is_loader_owned(entry) || is_loop_registered || is_scanned)
        })
        .map(|entry| format!("{:?} {}", entry.kind, entry.name))
        .collect();
    missing.sort_unstable();
    ensure!(
        missing.is_empty(),
        "inventory helpers with no registration site: {missing:?}"
    );
    Ok(())
}

/// Reject a registration whose name is inventoried only under another kind.
///
/// `MiniJinja` keeps functions, filters, and tests in separate namespaces, so
/// `SURFACE` may legitimately hold the same name twice — `which` is both a
/// function and a filter. The scan must therefore compare the kind as well as
/// the name; comparing names alone would accept a filter called `now` merely
/// because the inventoried `now` is a function.
#[test]
fn a_registration_is_not_inventoried_under_another_kind() {
    assert!(
        is_inventoried(HelperKind::Function, "now"),
        "the function `now` should be inventoried"
    );
    assert!(
        !is_inventoried(HelperKind::Filter, "now"),
        "a filter named `now` must not inherit the function's inventory row"
    );
    assert!(
        is_inventoried(HelperKind::Filter, "which"),
        "the filter `which` should be inventoried"
    );
    assert!(
        is_inventoried(HelperKind::Function, "which"),
        "the function `which` should be inventoried"
    );
}
