//! Read a Rust source file and resolve which of its implementations
//! `cargo-mutants` would mutate.
//!
//! This module holds the parsing and selection logic for the mutation-selection
//! contract; the contract's assertions live in
//! `tests/mutation_selection_contract_tests.rs`. Splitting them keeps parser
//! maintenance apart from the selection requirements, and keeps each file inside
//! the repository's 400-line limit.
//!
//! The rule modelled here is the one cargo-mutants 27.1.0 applies, not a
//! plausible simplification of it. Two details matter and both were measured:
//!
//! - `#[mutants::skip]` is honoured on an item, and inside a `cfg_attr` only
//!   when the condition is a **bare path**. Its reader parses `cfg_attr`
//!   arguments as nested metadata, so `cfg_attr(any(), mutants::skip)` is
//!   rejected outright and the skip is silently dropped.
//! - A skip on an *enclosing* scope removes everything inside it. Its walker
//!   returns early from `visit_file` and from `visit_item_impl` before it
//!   reaches the methods, so a skip on the `impl` block — or on the file, as an
//!   inner attribute — suppresses every method's mutants even though none of
//!   them carries an attribute of its own.
//!
//! Reading only a method's own attributes would therefore report a method as
//! selected while cargo-mutants skips it. [`implementations_by_name`] carries
//! the enclosing decision down to each method so the two cannot disagree.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use syn::{Attribute, File, ImplItem, Item, ItemImpl, Meta};

/// One implementation found in an `impl` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Implementation {
    /// The method name cargo-mutants would print for this implementation.
    pub name: String,
    /// True when this implementation's mutants would not be produced.
    ///
    /// This is the *effective* decision: it accounts for a skip on the method
    /// itself and for one inherited from the enclosing `impl` block or file.
    pub skipped: bool,
    /// True when the item is gated to the Kani runner.
    pub kani_only: bool,
}

/// What one method name resolves to across all its implementations.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Selection {
    /// Implementations of this name that stay in the ordinary lane.
    pub selected: usize,
    /// Implementations of this name that carry a skip, inherited or direct.
    pub skipped: usize,
    /// Skipped implementations that are not `cfg(kani)`.
    pub skipped_outside_kani: usize,
}

/// True when `attr` is, or contains as a bare-path condition, `mutants::skip`.
///
/// Mirrors the rule cargo-mutants applies. Its reader accepts an attribute path
/// of exactly `mutants::skip`, or a `cfg_attr` whose first argument is a bare
/// path and whose remaining arguments include `mutants::skip`. A `cfg_attr`
/// condition written with parentheses — `any()`, `all(kani, test)` — is not a
/// path, so the whole attribute is rejected and a skip written inside it is
/// silently dropped. Recognising only the accepted forms here is what stops
/// this contract from certifying a skip cargo-mutants would ignore.
fn is_mutants_skip(attr: &Attribute) -> bool {
    match &attr.meta {
        Meta::Path(path) => path_is_mutants_skip(path),
        Meta::List(list) => {
            if !path_is_cfg_attr(&list.path) {
                return false;
            }
            let mut skip = false;
            let parsed = attr.parse_nested_meta(|meta| {
                if path_is_mutants_skip(&meta.path) {
                    skip = true;
                }
                Ok(())
            });
            parsed.is_ok() && skip
        }
        Meta::NameValue(_) => false,
    }
}

/// True when `path` is exactly `mutants::skip`.
fn path_is_mutants_skip(path: &syn::Path) -> bool {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .eq(["mutants".to_owned(), "skip".to_owned()])
}

/// True when `path` is exactly `cfg_attr`.
fn path_is_cfg_attr(path: &syn::Path) -> bool {
    path.is_ident("cfg_attr")
}

/// True when `attrs` gate the item to the Kani runner.
///
/// A `cfg_attr` carrying a `kani` condition is deliberately not treated as a
/// Kani gate: the ordinary lane does not evaluate it, so such an item would be
/// present in the test binaries and must keep its mutants.
fn is_kani_only(attrs: &[Attribute]) -> bool {
    attrs.iter().any(is_direct_kani_cfg)
}

/// True when `attr` is a `cfg` attribute naming `kani` as one of its conditions.
fn is_direct_kani_cfg(attr: &Attribute) -> bool {
    let Meta::List(list) = &attr.meta else {
        return false;
    };
    if !list.path.is_ident("cfg") {
        return false;
    }
    let mut found = false;
    let parsed = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("kani") {
            found = true;
        }
        Ok(())
    });
    parsed.is_ok() && found
}

/// Collect every implementation in `source`, grouped by method name.
///
/// # Errors
///
/// Returns an error when `source` does not parse as a Rust file.
pub fn implementations_by_name(source: &str) -> Result<BTreeMap<String, Selection>> {
    let parsed: File = syn::parse_str(source).context("parse Rust source as a file")?;
    // A file-level `#![mutants::skip]` is an inner attribute on the file itself,
    // and cargo-mutants returns from `visit_file` before walking any item.
    let file_skipped = parsed.attrs.iter().any(is_mutants_skip);
    let mut by_name: BTreeMap<String, Selection> = BTreeMap::new();
    for item in &parsed.items {
        let Item::Impl(block) = item else {
            continue;
        };
        accumulate_impl(block, file_skipped, &mut by_name);
    }
    Ok(by_name)
}

/// Fold one `impl` block's methods into `by_name`.
///
/// `enclosing_skipped` carries the file-level decision; a skip on this block is
/// added to it, so a method inherits either.
fn accumulate_impl(
    block: &ItemImpl,
    enclosing_skipped: bool,
    by_name: &mut BTreeMap<String, Selection>,
) {
    let block_skipped = enclosing_skipped || block.attrs.iter().any(is_mutants_skip);
    for item in &block.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };
        let implementation = Implementation {
            name: method.sig.ident.to_string(),
            skipped: block_skipped || method.attrs.iter().any(is_mutants_skip),
            kani_only: is_kani_only(&method.attrs),
        };
        let entry = by_name.entry(implementation.name).or_default();
        if implementation.skipped {
            entry.skipped += 1;
            if !implementation.kani_only {
                entry.skipped_outside_kani += 1;
            }
        } else {
            entry.selected += 1;
        }
    }
}
