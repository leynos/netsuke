//! Contract test: no unordered collection is iterated inside the emitter.
//!
//! `OBL-NOHASH` states that the bytes Ninja output is built from never depend
//! on a hash iteration order. The properties in
//! `src/ninja_gen/determinism/property_tests/` pin the *rules* that impose
//! order, but a rule only covers the collection it was written for: a future
//! map keyed on pools, or variables, or anything the generator does not yet
//! produce would not be varied by any strategy, so no property would notice it.
//! This crate is the part that generalizes — it reads the emitter's own sources
//! and fails the day an unordered collection appears without an order imposed
//! before its contents are consumed.
//!
//! The scan is over *compiled* sources, not every file. Test-only code is
//! removed in two places, because Rust expresses it in two: an item carrying
//! `#[cfg(test)]` is stripped by the visitor, and a file named by a
//! `#[cfg(test)] mod` declaration is skipped whole by the walk. Both are
//! needed. The emitter's own property suites hold `HashMap` and `HashSet`, and
//! neither can reach an emitted byte because neither is compiled into a release
//! artefact; `src/ninja_gen/property_tests/mod.rs` is the case that makes the
//! second strip load-bearing, since its `use std::collections::{HashMap,
//! HashSet}` carries no attribute of its own and is test-only only by virtue of
//! the declaration that names it. Reading those files would force a path
//! allowlist, and an allowlist is the thing that rots — a path exemption
//! written for the test tree would equally excuse a production file that later
//! took the same name. Removing the declaration is the honest classification
//! instead.
//!
//! The exemption list is asserted in both directions. `staged_sidecars` is the
//! one unordered collection the emitter may hold, because it is only ever asked
//! whether a sidecar has been staged: membership, never iteration. A stale
//! entry — one that no longer holds a hash collection — fails as loudly as a
//! new site, because a rule that describes a tree this is not has stopped
//! being a rule.
//!
//! The reading of a source lives in `tests/hashmap_boundary/scan.rs`, which the
//! `#[path]` below wires in. That directory has no `mod.rs` on purpose: the
//! integration-wiring contract polices `tests/<name>/mod.rs` trees, and a
//! directory of helpers included by path is not one.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, DirEntry},
};

#[path = "hashmap_boundary/scan.rs"]
mod scan;

use scan::{Hit, Kind, UNORDERED_TYPES};

/// The emitter's source root, relative to the workspace root.
const NINJA_GEN: &str = "src/ninja_gen";

/// The fewest compiled sources the walk must find for its result to mean
/// anything.
///
/// A walk that descended nowhere, or stopped after one directory, would find
/// no offending collection and report a clean tree. The emitter carries far
/// more than this, so ordinary growth and pruning never trip it.
const MINIMUM_SOURCES: usize = 10;

/// The one unordered collection the emitter may hold, and why.
///
/// A pair rather than a bare field name: a field called `staged_sidecars` is
/// unique only within its module, so a future module declaring the same field
/// must be named here deliberately rather than inheriting the exemption. The
/// import that brings `HashSet` into that module is exempt with it, since the
/// import and the field are one decision about one type.
const PERMITTED_UNORDERED_FIELDS: [(&str, &str); 1] =
    [("staged_sidecars", "src/ninja_gen/dyndep/mod.rs")];

/// Return whether `hit` is covered by [`PERMITTED_UNORDERED_FIELDS`].
///
/// An import is exempt when its own *file* is the permitted file: the import is
/// how the permitted field's type arrives, and it cannot be iterated by itself.
/// Every other hit must name the permitted field.
fn is_permitted(hit: &Hit) -> bool {
    let source = hit.source.as_str();
    let permitted_source = PERMITTED_UNORDERED_FIELDS
        .iter()
        .any(|(_, permitted)| *permitted == source);
    match &hit.kind {
        Kind::Import => permitted_source,
        Kind::Field(Some(field)) => PERMITTED_UNORDERED_FIELDS
            .iter()
            .any(|(permitted, owner)| *permitted == field.as_str() && *owner == source),
        Kind::Field(None) | Kind::Other => false,
    }
}

/// Join `name` to `prefix` with a forward slash.
///
/// Every path this module compares against is written with `/`, and
/// [`Utf8Path::join`] would insert the platform's separator instead, so on
/// Windows the exemption list would match nothing and every entry would look
/// stale. Naming the separator keeps one spelling of a repository path.
fn contract_path(prefix: &Utf8Path, name: &str) -> String {
    format!("{prefix}/{name}")
}

/// One compiled-away-capable source: its path, text, and how it was declared.
struct Source {
    /// Workspace-relative path of the source.
    path: String,
    /// The source's text, as read.
    text: String,
    /// Whether the `mod` declaration that named this file is test-only.
    ///
    /// The crate root is not named by any `mod`, so it is never test-only.
    declaration_is_test_only: bool,
}

/// Collect every `.rs` source beneath `directory`, as [`Source`]s.
///
/// Each directory is entered from its own handle, opened relative to the one
/// above it, so the walk carries no capability it does not use. The order is
/// the filesystem's; nothing here depends on it, because each source is read in
/// full and the findings are sorted before they are compared.
///
/// `is_test_only` and `sibling_declarations` are both about *this* directory,
/// supplied by whoever opened it. The flag covers `mod.rs` itself, whose
/// declaring `mod` lives one level up. The sibling text covers a directory
/// whose children are declared by a file beside it rather than by a `mod.rs`
/// inside it — `property_tests/defaults/` was declared that way before the
/// directory gained a `mod.rs` of its own, and the sibling path is still taken
/// for `tests/hashmap_boundary/`, which declares no `mod.rs`.
fn collect_sources(
    directory: &Dir,
    prefix: &Utf8Path,
    is_test_only: bool,
    sibling_declarations: Option<String>,
) -> Result<Vec<Source>> {
    let declarations = Declarations::read(directory, prefix, sibling_declarations)?;
    let context = DirectoryContext {
        is_test_only,
        declarations: &declarations,
    };
    let mut sources = Vec::new();
    for entry in directory
        .read_dir(".")
        .with_context(|| format!("read {prefix}"))?
    {
        sources.extend(collect_entry_sources(directory, prefix, &entry?, &context)?);
    }
    Ok(sources)
}

/// What a directory's own entries need to know about it.
///
/// The two answers travel together: both are properties of the directory being
/// walked rather than of the entry being classified, and both are supplied by
/// whoever opened it. Grouping them keeps the per-entry helper's signature about
/// the entry.
struct DirectoryContext<'a> {
    /// Whether the `mod` declaration naming this directory is test-only.
    is_test_only: bool,
    /// The `mod` declarations this directory's children are named by.
    declarations: &'a Declarations,
}

/// Collect the sources one directory entry contributes, under `prefix`.
///
/// A directory contributes whatever the walk finds beneath it, a `.rs` file
/// contributes itself, and anything else contributes nothing. A file's
/// test-only-ness comes from the declaration that named it: `mod.rs` from the
/// flag in `context`, every sibling from this directory's own declaring file.
fn collect_entry_sources(
    directory: &Dir,
    prefix: &Utf8Path,
    entry: &DirEntry,
    context: &DirectoryContext<'_>,
) -> Result<Vec<Source>> {
    let name = entry.file_name().context("read entry name")?;
    let path = contract_path(prefix, &name);
    let file_type = entry.file_type().context("read entry type")?;
    if file_type.is_dir() {
        let nested = directory
            .open_dir(&name)
            .with_context(|| format!("open {path}"))?;
        // A directory with no `mod.rs` of its own may still have its children
        // declared by the file of the same name beside it.
        let sibling = directory.read_to_string(format!("{name}.rs")).ok();
        return collect_sources(
            &nested,
            Utf8Path::new(&path),
            context.is_test_only || context.declarations.declares_test_only(&name),
            sibling,
        );
    }
    if !file_type.is_file() || Utf8Path::new(&name).extension() != Some("rs") {
        return Ok(Vec::new());
    }
    let text = directory
        .read_to_string(&name)
        .with_context(|| format!("read {path}"))?;
    let stem = Utf8Path::new(&name).file_stem().unwrap_or(&name);
    let declaration_is_test_only = if name == "mod.rs" {
        context.is_test_only
    } else {
        context.is_test_only || context.declarations.declares_test_only(stem)
    };
    Ok(vec![Source {
        path,
        text,
        declaration_is_test_only,
    }])
}

/// The `mod` declarations one directory's sources are named by.
struct Declarations {
    /// The parsed declaring file, when one could be read and parsed.
    file: Option<syn::File>,
}

impl Declarations {
    /// Read a directory's declaring file.
    ///
    /// `mod.rs` wins when present; otherwise the sibling text handed in by the
    /// parent is used. A directory with neither declares nothing, and every
    /// child of it is treated as an ordinary source rather than excused: a
    /// silent `false` here would be a hole in the contract, not a convenience.
    fn read(directory: &Dir, prefix: &Utf8Path, sibling: Option<String>) -> Result<Self> {
        let (text, path) = directory.read_to_string("mod.rs").map_or_else(
            |_| (sibling, format!("{prefix}.rs")),
            |text| (Some(text), contract_path(prefix, "mod.rs")),
        );
        let file = text
            .map(|source| scan::parse(&source).with_context(|| format!("parse {path}")))
            .transpose()?;
        Ok(Self { file })
    }

    /// Return whether this directory declares `name` only for tests.
    fn declares_test_only(&self, name: &str) -> bool {
        self.file
            .as_ref()
            .is_some_and(|file| scan::declares_test_only_module(file, name))
    }
}

/// Every unordered collection in the emitter must be the permitted one.
///
/// Reported as a set equality in both directions. An unlisted site is an
/// iteration order that can reach an emitted byte; a listed site that no
/// longer holds a collection is a stale exemption, which is how a rule like
/// this rots into a description of a tree that no longer exists. Neither is
/// visible from the other's assertion, so both are made here.
#[test]
fn only_membership_use_of_unordered_collections_reaches_the_emitter() -> Result<()> {
    let workspace = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = Dir::open_ambient_dir(&workspace, ambient_authority())
        .context("open the workspace root")?;
    let emitter = Utf8Path::new(NINJA_GEN);
    let emitter_dir = root
        .open_dir(emitter)
        .with_context(|| format!("open {NINJA_GEN}"))?;

    let sources = collect_sources(&emitter_dir, emitter, false, None)?;
    ensure!(
        sources.len() >= MINIMUM_SOURCES,
        "the walk found {} sources under {NINJA_GEN}, fewer than the {MINIMUM_SOURCES} \
         a working walk would see; the result would be vacuously clean",
        sources.len()
    );

    let mut found = Vec::new();
    for source in &sources {
        if source.declaration_is_test_only {
            continue;
        }
        let parsed = scan::parse(&source.text).with_context(|| format!("parse {}", source.path))?;
        found.extend(scan::collect(&source.path, &parsed));
    }
    found.sort();
    found.dedup();

    let unexpected: Vec<&Hit> = found.iter().filter(|hit| !is_permitted(hit)).collect();
    ensure!(
        unexpected.is_empty(),
        "every unordered collection under {NINJA_GEN} must be membership-only and \
         declared in PERMITTED_UNORDERED_FIELDS; these are not: {unexpected:?}. \
         Impose a total order before consuming the values, or use an ordered \
         collection, rather than letting a hash iteration order reach the \
         emitted file."
    );

    let observed: BTreeSet<(&str, &str)> = found
        .iter()
        .filter_map(|hit| hit.kind.field().map(|field| (field, hit.source.as_str())))
        .collect();
    let stale: Vec<&(&str, &str)> = PERMITTED_UNORDERED_FIELDS
        .iter()
        .filter(|permitted| !observed.contains(*permitted))
        .collect();
    ensure!(
        stale.is_empty(),
        "these sites are permitted to hold an unordered collection but no longer \
         do: {stale:?}. Drop the permission rather than leaving a rule that \
         describes a tree this is not."
    );
    Ok(())
}

/// The type list the scan matches is the list the contract means.
///
/// The ordered collections are the ones a future author should reach for, so
/// the rule must not name them; and a name that merely starts with a matched
/// spelling is a different type. Both are asserted so widening the list is a
/// deliberate edit rather than a side effect of a refactor.
#[test]
fn the_matched_type_list_names_only_unordered_collections() {
    assert_eq!(
        UNORDERED_TYPES,
        ["HashMap", "HashSet"],
        "the contract is stated over the unordered collection types, and the \
         ordered ones already impose the order it asks for"
    );
}

/// The scan sees the shapes the emitter could adopt, not only today's.
///
/// The emitter-side test can only report what the scan can see, so a source
/// shape the scan cannot reach is a hole no amount of correct emitter code
/// closes. These cases name a shape and assert a hit, so narrowing the visitor
/// fails here rather than silently widening what the contract permits.
mod scan_coverage {
    use super::scan::{Kind, collect, parse};

    /// Every unordered type named by a constructor is found, including one
    /// whose type is inferred and one written with a turbofish.
    #[test]
    fn an_expression_path_reaches_the_scan() {
        let source = "
            fn build() {
                let inferred = std::collections::HashMap::new();
                let qualified = std::collections::HashMap::<String, u8>::new();
                let _ = (inferred, qualified);
            }
        ";
        let parsed = parse(source).expect("the case is a valid source");
        let hits = collect("case.rs", &parsed);
        let constructors: Vec<&str> = hits
            .iter()
            .filter(|hit| hit.kind == Kind::Other)
            .map(|hit| hit.type_name.as_str())
            .collect();
        assert_eq!(
            constructors,
            ["HashMap", "HashMap"],
            "both the inferred and the turbofished constructor are uses"
        );
    }

    /// A collection named inside a test-only item is not a hit.
    ///
    /// The counterpart to the case above: widening the visitor must not widen
    /// what counts as reaching the emitter.
    #[test]
    fn a_test_only_expression_path_is_ignored() {
        let source = "
            #[cfg(test)]
            mod tests {
                fn build() {
                    let _ = std::collections::HashMap::new();
                }
            }
        ";
        let parsed = parse(source).expect("the case is a valid source");
        assert!(
            collect("case.rs", &parsed).is_empty(),
            "a release build never compiles the test module"
        );
    }
}
