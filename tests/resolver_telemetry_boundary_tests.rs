//! Contract test: the resolver domain does not depend on its telemetry layer.
//!
//! [`crate::stdlib::which::resolve_error`] describes what can go wrong while
//! resolving a command. Its `category()` used to return a `&'static str` read
//! straight out of the telemetry module's label constants, which made the
//! domain's notion of a failure and the label a metric carries the same
//! declaration. Nothing failed when that happened — every test still passed,
//! because the taxonomy *was* the label set — and that is exactly the problem:
//! renaming a metric label would silently rename a domain concept, and a
//! reader of the error type could not tell which of the two they were looking
//! at. The two are now separate — the domain owns `ResolveErrorCategory` and
//! the telemetry boundary maps it — and this crate holds that separation in
//! place.
//!
//! The assertion is about source text, which is normally the wrong shape. An
//! `use` is source text, there is no execution to model, and "this module does
//! not reach into that one" is a statement about what the module says. The
//! other direction — that the two taxonomies still agree on every spelling —
//! is an executable property and is asserted in `telemetry_tests` instead.
//!
//! The scan is a whole-set equality rather than a deny-list: the modules that
//! may name telemetry are enumerated, and the set found on disk must equal it.
//! A deny-list would pass when a *new* domain module reached into telemetry,
//! which is the failure this exists to catch.
//!
//! Read at [`mask_non_code`]'s discretion: a `use` inside a comment or a
//! literal is not an import, and a scan that read the module's own prose as
//! code would report the module discussing the rule as breaking it.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

/// The resolver domain, relative to the workspace root.
const RESOLVER_DOMAIN: &str = "src/stdlib/which";

/// The fewest sources the walk must find for its result to mean anything.
///
/// A walk that descended nowhere, or stopped after one directory, would find
/// no importer at all and report a clean tree. The resolver domain is larger
/// than this by some margin, so ordinary growth and pruning never trip it.
const MINIMUM_DOMAIN_SOURCES: usize = 5;

/// The boundary module itself, which by definition does not import itself.
///
/// It is excluded from the importer set rather than listed in it: the scan asks
/// which modules *reach into* telemetry, and this one is the thing reached
/// into. Its own imports point the other way — it names the domain taxonomy, so
/// that the mapping from category to label lives on this side of the boundary.
const TELEMETRY_BOUNDARY_SOURCE: &str = "src/stdlib/which/telemetry.rs";

/// Sources that may name the telemetry module, and why.
///
/// Enumerated rather than derived: the point of the list is that each entry is
/// a decision someone made, and a new name has to be added deliberately. The
/// three roles are distinct — the boundary's caller, the module that declares
/// it, and the tests that hold it to its contract — and no fourth role exists
/// in this domain.
const PERMITTED_TELEMETRY_IMPORTERS: [&str; 4] = [
    // The resolver's caller of the boundary. Records spans and counts; it
    // names the label constants it records, which is the boundary's public
    // surface rather than the domain's internals.
    "src/stdlib/which/cache.rs",
    // Declares the module and re-exports the vocabularies the application
    // recorder admits on.
    "src/stdlib/which/mod.rs",
    // The tests that assert the boundary's contract, including the one that
    // pins the mapping between the two taxonomies.
    "src/stdlib/which/telemetry_tests.rs",
    // The counter-series cases those tests are split into.
    "src/stdlib/which/telemetry_tests/outcome_series.rs",
];

/// The submodules of `telemetry_tests` are permitted wholesale.
///
/// They are test cases for the telemetry boundary, so naming it is their
/// subject rather than a leak; listing each one by hand would mean editing
/// this file every time the tests are split further, for a decision that was
/// already made when the directory was allowed.
const PERMITTED_TELEMETRY_TEST_DIRECTORY: &str = "src/stdlib/which/telemetry_tests/";

/// Return whether `path` may name the telemetry module.
fn is_permitted(path: &str) -> bool {
    PERMITTED_TELEMETRY_IMPORTERS.contains(&path)
        || path.starts_with(PERMITTED_TELEMETRY_TEST_DIRECTORY)
}

/// Return whether `needle` sits at `index` in `source`.
fn matches_at(source: &[u8], index: usize, needle: &[u8]) -> bool {
    source.get(index..index + needle.len()) == Some(needle)
}

/// Return the offset of the next `byte` at or after `index`.
fn find_byte(source: &[u8], byte: u8, index: usize) -> Option<usize> {
    source
        .get(index..)?
        .iter()
        .position(|found| *found == byte)
        .map(|found| index + found)
}

/// Return the end of a line comment beginning at `index`, when present.
fn line_comment_end(source: &[u8], index: usize) -> Option<usize> {
    if !matches_at(source, index, b"//") {
        return None;
    }
    Some(find_byte(source, b'\n', index).unwrap_or(source.len()))
}

/// Return the end of a possibly nested block comment at `index`, when present.
fn block_comment_end(source: &[u8], index: usize) -> Option<usize> {
    if !matches_at(source, index, b"/*") {
        return None;
    }
    let (mut depth, mut end) = (1_usize, index + 2);
    while depth > 0 && end < source.len() {
        if matches_at(source, end, b"/*") {
            depth += 1;
            end += 2;
        } else if matches_at(source, end, b"*/") {
            depth -= 1;
            end += 2;
        } else {
            end += 1;
        }
    }
    Some(end)
}

/// Return the end of a string or character literal at `index`, when present.
///
/// A single quote opens a literal only when one closes it on the same line, so
/// a lifetime (`&'static str`) or a loop label is not read as one.
fn literal_end(source: &[u8], index: usize) -> Option<usize> {
    let quote = *source.get(index)?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    let mut end = index + 1;
    while let Some(byte) = source.get(end).copied() {
        match byte {
            b'\\' => end += 2,
            _ if byte == quote => return Some(end + 1),
            b'\n' if quote == b'\'' => return None,
            _ => end += 1,
        }
    }
    Some(end)
}

/// Return the end of a raw Rust string beginning at `index`, when present.
fn raw_string_end(source: &[u8], index: usize) -> Option<usize> {
    let mut start = index;
    if source.get(start) == Some(&b'b') {
        start += 1;
    }
    if source.get(start) != Some(&b'r') {
        return None;
    }
    let mut hashes = 0;
    while source.get(start + 1 + hashes) == Some(&b'#') {
        hashes += 1;
    }
    if source.get(start + 1 + hashes) != Some(&b'"') {
        return None;
    }
    let body = start + 2 + hashes;
    // The terminator is a quote followed by as many hashes as opened it.
    let mut end = body;
    while source.get(end).is_some() {
        if source.get(end).copied() == Some(b'"')
            && (0..hashes).all(|offset| source.get(end + 1 + offset) == Some(&b'#'))
        {
            return Some(end + 1 + hashes);
        }
        end += 1;
    }
    Some(source.len())
}

/// Return the end of non-code text beginning at `index`, when present.
///
/// Order matters: a raw string may open with `b` or `r`, and a line comment
/// begins with a byte a literal scan would otherwise treat as ordinary code.
fn non_code_end(source: &[u8], index: usize) -> Option<usize> {
    line_comment_end(source, index)
        .or_else(|| block_comment_end(source, index))
        .or_else(|| raw_string_end(source, index))
        .or_else(|| literal_end(source, index))
}

/// Blank `span` in place, keeping its newlines so line structure survives.
fn blank(span: &mut [u8]) {
    for byte in span.iter_mut().filter(|byte| **byte != b'\n') {
        *byte = b' ';
    }
}

/// Replace comments and literals with spaces, preserving every byte offset.
///
/// Offsets are preserved so a match's position still maps to the source it came
/// from, and so the masking can be done on bytes: Rust source carries
/// multi-byte prose in its doc comments, and a char-wise blanking would shorten
/// the text and shift every offset after the first em-dash.
fn mask_non_code(source: &str) -> Vec<u8> {
    let mut masked = source.as_bytes().to_vec();
    let mut index = 0;
    while index < source.len() {
        let Some(span_end) = non_code_end(&masked, index).filter(|end| *end > index) else {
            // Not the start of anything masked, so this byte is code and stays.
            index += 1;
            continue;
        };
        // `get_mut` rather than an index, so a range the scan cannot produce
        // blanks nothing instead of panicking.
        if let Some(span) = masked.get_mut(index..span_end) {
            blank(span);
        }
        index = span_end;
    }
    masked
}

/// Return whether `statement` carries `token` as a whole word.
fn names_token(statement: &[u8], token: &[u8]) -> bool {
    statement
        .windows(token.len())
        .enumerate()
        .any(|(index, window)| {
            let before_is_word = index
                .checked_sub(1)
                .and_then(|previous| statement.get(previous))
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
            let after = statement.get(index + token.len());
            let after_is_word =
                after.is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
            window == token && !before_is_word && !after_is_word
        })
}

/// Return whether masked `source` names the telemetry module in a `use`.
///
/// The unit examined is a statement rather than a line, because an import that
/// names telemetry need not be written on the line that opens it — the module
/// tree here spells most of them as a braced group spanning four lines — and
/// because `mod.rs` re-exports with `pub use`, which a `use `-prefix test would
/// miss.
///
/// Only `use` is examined, not qualified paths written at a call site. Those
/// are the same dependency, and the rule is stated over imports, so a module
/// that names `telemetry` without importing it would slip past; the resolver
/// therefore imports rather than qualifies, which is the convention this
/// encodes. It is a convention rather than an invariant because a qualified
/// path is not distinguishable from any other path expression by text alone.
fn names_telemetry_in_a_use(masked: &[u8]) -> bool {
    masked
        .split(|byte| *byte == b';')
        .any(|statement| names_token(statement, b"use") && names_token(statement, b"telemetry"))
}

/// Collect every `.rs` source beneath `directory`, as workspace-relative paths.
///
/// Each directory is entered from its own handle — the ambient root is opened
/// once by the caller and never traversed — so the walk carries no capability
/// it does not use, and `prefix` names the position in the workspace the
/// resulting paths are reported under.
fn collect_sources(directory: &Dir, prefix: &Utf8Path) -> Result<Vec<(String, String)>> {
    let mut sources = Vec::new();
    for entry in directory
        .read_dir(".")
        .with_context(|| format!("read {prefix}"))?
    {
        let handle = entry.with_context(|| format!("read an entry of {prefix}"))?;
        let name = handle.file_name().context("read entry name")?;
        let child = prefix.join(&name);
        let file_type = handle.file_type().context("read entry type")?;
        if file_type.is_dir() {
            let nested = directory
                .open_dir(&name)
                .with_context(|| format!("open {child}"))?;
            sources.extend(collect_sources(&nested, &child)?);
        } else if file_type.is_file() && Utf8Path::new(&name).extension() == Some("rs") {
            let text = directory
                .read_to_string(&name)
                .with_context(|| format!("read {child}"))?;
            sources.push((child.to_string(), text));
        }
    }
    Ok(sources)
}

/// Every module under the resolver domain that names telemetry must be allowed to.
///
/// Reported as a set equality in both directions. An unlisted importer is the
/// domain reaching into its own reporting layer; a listed path that no longer
/// imports anything is a stale permission, which is how a rule like this rots
/// into a description of a tree that no longer exists. Neither is visible from
/// the other's assertion, so both are made here.
#[test]
fn only_the_telemetry_boundary_names_telemetry_in_the_resolver_domain() -> Result<()> {
    let workspace = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = Dir::open_ambient_dir(&workspace, ambient_authority())
        .context("open the workspace root")?;
    let domain = Utf8Path::new(RESOLVER_DOMAIN);
    let domain_dir = root
        .open_dir(domain)
        .with_context(|| format!("open {RESOLVER_DOMAIN}"))?;

    let sources = collect_sources(&domain_dir, domain)?;
    ensure!(
        sources.len() >= MINIMUM_DOMAIN_SOURCES,
        "the walk found {} sources under {RESOLVER_DOMAIN}, fewer than the \
         {MINIMUM_DOMAIN_SOURCES} a working walk would see; the result would \
         be vacuously clean",
        sources.len()
    );

    let mut importers = BTreeSet::new();
    for (path, text) in &sources {
        if path != TELEMETRY_BOUNDARY_SOURCE && names_telemetry_in_a_use(&mask_non_code(text)) {
            importers.insert(path.clone());
        }
    }

    let unexpected: Vec<&String> = importers
        .iter()
        .filter(|path| !is_permitted(path))
        .collect();
    ensure!(
        unexpected.is_empty(),
        "every module under {RESOLVER_DOMAIN} that names telemetry must be a \
         boundary between the domain and its reporting layer; these are not: \
         {unexpected:?}. The resolver's error taxonomy is its own \
         (`ResolveErrorCategory` in `resolve_error.rs`); map it to a label at \
         the telemetry boundary rather than importing the label into the \
         domain."
    );

    // The stale-permission direction, checked over the declared list rather
    // than over the sources so a permission naming a file that no longer
    // exists is caught too.
    let stale: Vec<&str> = PERMITTED_TELEMETRY_IMPORTERS
        .iter()
        .copied()
        .filter(|path| !importers.contains(*path))
        .collect();
    ensure!(
        stale.is_empty(),
        "these sources are permitted to name telemetry but no longer do: \
         {stale:?}. Drop the permission rather than leaving a rule that \
         describes a tree this is not."
    );
    Ok(())
}
