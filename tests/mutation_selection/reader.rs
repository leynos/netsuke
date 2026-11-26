//! Read a Rust source file and resolve which of its implementations
//! `cargo-mutants` would mutate.
//!
//! This module holds the parsing and selection logic for the mutation-selection
//! contract; the contract's assertions live in
//! `tests/mutation_selection_contract_tests.rs`, and the probes for the rules
//! below live in the `tests` child module. Splitting them keeps parser
//! maintenance apart from the selection requirements, and keeps each file inside
//! the repository's 400-line limit.
//!
//! The rule modelled here is the one cargo-mutants 27.1.0 applies, not a
//! plausible simplification of it. Five details matter, and all were measured
//! against the pinned tool:
//!
//! - `#[mutants::skip]` is matched on its attribute **path**, so the
//!   parenthesized spellings `#[mutants::skip()]` and `#[mutants::skip(..)]`
//!   are honoured too; the `mutants` crate's attribute ignores its arguments.
//! - Inside a `cfg_attr` the skip is honoured only when the condition is a
//!   **bare path**. The tool parses `cfg_attr` arguments as nested metadata, so
//!   `cfg_attr(any(), mutants::skip)` is rejected outright and the skip is
//!   silently dropped.
//! - An *enclosing* exclusion removes everything inside it. The walker returns
//!   early from `visit_file`, `visit_item_mod` and `visit_item_impl` before it
//!   reaches the methods, so an exclusion on the file, on an inline module, or
//!   on the `impl` block suppresses every method's mutants even though none of
//!   them carries an attribute of its own.
//! - `#[mutants::skip]` is not the only exclusion. The walker's
//!   `attrs_excluded` also matches a bare `#[cfg(test)]` and any attribute whose
//!   path ends with `test`. The `cfg` gate is narrow: only a `test` at the
//!   cfg's *top level* counts, so a `test` nested in `all(..)` or `any(..)` is
//!   still mutated.
//! - A method can be *ineligible* for reasons other than an attribute. The
//!   walker skips an `unsafe fn`, a constructor named `new`, and a method whose
//!   body is empty, because none of them yields a viable or useful mutant.
//!
//! Reading only a method's own `#[mutants::skip]` would therefore report a
//! method as selected while cargo-mutants excludes it. [`implementations_by_name`]
//! carries every enclosing decision and every eligibility check down to each
//! method so the two cannot disagree.

use std::collections::BTreeMap;

use crate::mutation_selection::cfg_predicate::Verdict;
use anyhow::{Context, Result};
use syn::spanned::Spanned;
use syn::{Attribute, File, ImplItem, ImplItemFn, Item, ItemImpl, ItemMod, Meta, Signature};

/// One implementation found in an `impl` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Implementation {
    /// The method name cargo-mutants would print for this implementation.
    pub name: String,
    /// The type the method is implemented for.
    ///
    /// Kept alongside the name because a bare method name does not identify an
    /// implementation: two types can each declare `output_index`, and a skip on
    /// one would otherwise be counted against the other.
    pub owner: String,
    /// True when this implementation produces no mutants at all.
    ///
    /// This is the *effective* decision: it accounts for a skip on the method
    /// itself, one inherited from the enclosing `impl` block, module or file,
    /// and the walker's eligibility checks.
    pub skipped: bool,
    /// What the model can say about the item's `cfg` conditions.
    pub cfg: Verdict,
}

/// What one method name resolves to across all its implementations.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Selection {
    /// Implementations of this name that stay in the ordinary lane.
    pub selected: usize,
    /// Implementations of this name that are selected *and* not Kani-only.
    ///
    /// A production counterpart is the thing that keeps a name worth mutating,
    /// so a check for one has to exclude a selected `cfg(kani)` implementation.
    /// Counting those would pass a name whose only remaining implementation is
    /// proof-only.
    pub selected_production: usize,
    /// Implementations of this name that carry a skip, inherited or direct.
    pub skipped: usize,
    /// Skipped implementations that are not proven `cfg(kani)`.
    pub skipped_outside_kani: usize,
    /// Implementations whose `cfg` conditions the model could not decide.
    ///
    /// Any occurrence is a failure, not a pass: an undecidable condition means
    /// the reader cannot tell proof-only code from ordinary code, so the
    /// contract must refuse to certify either.
    pub undecidable: usize,
}

/// True when `attrs` exclude the node from mutation altogether.
///
/// Mirrors the tool's `attrs_excluded`: `#[mutants::skip]`, a bare
/// `#[cfg(test)]`, or any attribute whose path ends with `test`.
fn is_excluded(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(|attr| is_mutants_skip(attr) || is_cfg_test(attr) || path_ends_with_test(attr))
}

/// True when `attr` is, or contains as a bare-path condition, `mutants::skip`.
///
/// The path check comes first and does not require the `Meta::Path` form, so
/// `#[mutants::skip]`, `#[mutants::skip()]` and `#[mutants::skip(..)]` all
/// match — measured against the pinned tool, an `impl` marked
/// `#[mutants::skip()]` produces no mutants for any of its methods.
///
/// The `cfg_attr` arm is narrower. A `cfg_attr` condition written with
/// parentheses — `any()`, `all(kani, test)` — is not a path, and the tool's
/// `parse_nested_meta` rejects the whole attribute, so a skip written inside it
/// is silently dropped. Recognising only the accepted forms here is what stops
/// this contract from certifying a skip cargo-mutants would ignore.
fn is_mutants_skip(attr: &Attribute) -> bool {
    if path_is_mutants_skip(attr.path()) {
        return true;
    }
    let Meta::List(list) = &attr.meta else {
        return false;
    };
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

/// True when `attr`'s path ends with `test`, matching `#[test]`, `#[tokio::test]`.
fn path_ends_with_test(attr: &Attribute) -> bool {
    attr.path()
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "test")
}

/// True when `attr` is a `cfg` attribute naming `test` among its conditions.
///
/// Only a *top-level* `test` counts: the tool reads the cfg's own conditions
/// with `parse_nested_meta`, so a `test` nested in `all(..)` or `any(..)` is
/// not seen. Measured: `#[cfg(test)]` yields no mutant while
/// `#[cfg(all(test, unix))]` and `#[cfg(any(test, feature = "x"))]` both do.
fn is_cfg_test(attr: &Attribute) -> bool {
    let Meta::List(list) = &attr.meta else {
        return false;
    };
    if !list.path.is_ident("cfg") {
        return false;
    }
    let mut found = false;
    let parsed = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("test") {
            found = true;
        }
        Ok(())
    });
    parsed.is_ok() && found
}

/// The `cfg` predicates `attrs` impose, or `None` if any is unreadable.
///
/// A `cfg_attr` expands to whatever it names, and this model does not follow
/// that expansion, so its presence makes the set unreadable rather than being
/// skipped: skipping one could hide a condition that removes the item from the
/// ordinary build.
fn cfg_predicates(attrs: &[Attribute]) -> Option<Vec<Meta>> {
    cfg_predicate::predicates(attrs)
}

/// True when the walker would skip this method for a reason other than an
/// exclusion attribute.
///
/// Three eligibility checks apply: an `unsafe fn` yields no viable mutant, a
/// constructor named `new` is skipped deliberately, and a method whose body is
/// empty has nothing to mutate. All three were measured against the pinned
/// tool.
fn is_ineligible(method: &ImplItemFn) -> bool {
    is_ineligible_signature(&method.sig) || method.block.stmts.is_empty()
}

/// True when `sig` alone makes a method ineligible.
fn is_ineligible_signature(sig: &Signature) -> bool {
    sig.unsafety.is_some() || sig.ident == "new"
}

/// The decisions inherited by everything declared inside a scope.
///
/// Both are properties of the *enclosing* declarations rather than of the
/// method, and each is the OR of every enclosing scope's own answer: an item is
/// excluded if anything around it is, and is Kani-only if anything around it
/// requires Kani. Carrying them as one value keeps the recursion from growing a
/// parameter per rule.
#[derive(Clone, Default)]
struct Scope {
    /// Some enclosing scope excludes its contents from mutation.
    excluded: bool,
    /// Every enclosing scope's `cfg` predicates.
    ///
    /// Kept as predicates rather than as verdicts so the whole conjunction can
    /// be evaluated as one formula. Nested conditions must all hold together,
    /// and their atoms correlate: `cfg(any(kani, unix))` around
    /// `cfg(any(kani, not(unix)))` needs `unix` and its negation at once, so the
    /// pair requires Kani even though neither half does on its own.
    cfg: Vec<Meta>,
    /// True when an enclosing scope had a `cfg` the model could not read.
    cfg_unreadable: bool,
}

impl Scope {
    /// Return the scope inside `attrs`, inheriting `self`.
    fn nested(mut self, attrs: &[Attribute]) -> Self {
        self.excluded = self.excluded || is_excluded(attrs);
        match cfg_predicates(attrs) {
            Some(predicates) => self.cfg.extend(predicates),
            None => self.cfg_unreadable = true,
        }
        self
    }

    /// The verdict for everything gathered so far.
    fn verdict(&self) -> Verdict {
        if self.cfg_unreadable {
            Verdict::Undecidable
        } else {
            cfg_predicate::verdict(&self.cfg)
        }
    }
}

/// Collect every implementation in `source`, grouped by method name.
///
/// # Errors
///
/// Returns an error when `source` does not parse as a Rust file.
pub fn implementations_by_name(source: &str) -> Result<BTreeMap<String, Selection>> {
    let parsed: File = syn::parse_str(source).context("parse Rust source as a file")?;
    // A file-level inner attribute such as `#![mutants::skip]` — or a
    // crate-wide `#![cfg(kani)]` — is on the file itself, and cargo-mutants
    // returns from `visit_file` before walking any item.
    let file_scope = Scope::default().nested(&parsed.attrs);
    let mut by_name: BTreeMap<String, Selection> = BTreeMap::new();
    collect_items(&parsed.items, &file_scope, &mut by_name);
    Ok(by_name)
}

/// Fold a list of items into `by_name`, recursing through inline modules.
fn collect_items(items: &[Item], scope: &Scope, by_name: &mut BTreeMap<String, Selection>) {
    for item in items {
        match item {
            Item::Impl(block) => accumulate_impl(block, scope, by_name),
            Item::Mod(module) => accumulate_module(module, scope, by_name),
            _ => {}
        }
    }
}

/// Fold one inline module's items into `by_name`, inheriting its decisions.
fn accumulate_module(module: &ItemMod, scope: &Scope, by_name: &mut BTreeMap<String, Selection>) {
    // `mod foo;` has no inline content: its items live in another file, which
    // this reader does not follow. `cargo-mutants` does, so a source with such
    // a declaration is outside what this model can say anything about.
    let Some((_, items)) = &module.content else {
        return;
    };
    collect_items(items, &scope.clone().nested(&module.attrs), by_name);
}

/// The name of the type an `impl` block is for.
///
/// A path type yields its last segment, which is what cargo-mutants prints. A
/// non-path type — a reference, a tuple, a generic binder — has no such name,
/// and the model has no token printer available to render one. Those blocks are
/// keyed by their source line instead, which keeps two distinct shapes from
/// colliding under one empty key without claiming a name they do not have.
fn self_ty_name(block: &ItemImpl) -> String {
    if let syn::Type::Path(path) = &*block.self_ty
        && let Some(segment) = path.path.segments.last()
    {
        return segment.ident.to_string();
    }
    // `proc_macro2::Span` exposes a source location only through its `Debug`
    // rendering, which is stable enough to tell two blocks apart and is never
    // asserted on.
    format!("<impl at {:?}>", block.self_ty.span())
}

/// Fold one `impl` block's methods into `by_name`.
///
/// An exclusion or a Kani gate on this block joins whatever the file and
/// modules already decided, so a method inherits them all.
fn accumulate_impl(block: &ItemImpl, scope: &Scope, by_name: &mut BTreeMap<String, Selection>) {
    let block_scope = scope.clone().nested(&block.attrs);
    for item in &block.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };
        let method_scope = block_scope.clone().nested(&method.attrs);
        let implementation = Implementation {
            name: method.sig.ident.to_string(),
            owner: self_ty_name(block),
            skipped: method_scope.excluded || is_ineligible(method),
            cfg: method_scope.verdict(),
        };
        let key = format!("{}::{}", implementation.owner, implementation.name);
        let entry = by_name.entry(key).or_default();
        if implementation.cfg == Verdict::Undecidable {
            entry.undecidable += 1;
        }
        if implementation.skipped {
            entry.skipped += 1;
            if implementation.cfg != Verdict::KaniOnly {
                entry.skipped_outside_kani += 1;
            }
        } else {
            entry.selected += 1;
            if implementation.cfg == Verdict::Production {
                entry.selected_production += 1;
            }
        }
    }
}

mod cfg_predicate;

#[cfg(test)]
mod identity_tests;
#[cfg(test)]
mod scope_tests;
#[cfg(test)]
mod tests;
