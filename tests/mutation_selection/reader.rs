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

use anyhow::{Context, Result};
use syn::{Attribute, File, ImplItem, ImplItemFn, Item, ItemImpl, ItemMod, Meta, Signature};

/// One implementation found in an `impl` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Implementation {
    /// The method name cargo-mutants would print for this implementation.
    pub name: String,
    /// True when this implementation produces no mutants at all.
    ///
    /// This is the *effective* decision: it accounts for a skip on the method
    /// itself, one inherited from the enclosing `impl` block, module or file,
    /// and the walker's eligibility checks.
    pub skipped: bool,
    /// True when the item is gated to the Kani runner.
    pub kani_only: bool,
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
    /// Skipped implementations that are not `cfg(kani)`.
    pub skipped_outside_kani: usize,
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

/// True when `attrs` gate the item to the Kani runner.
///
/// A `cfg_attr` carrying a `kani` condition is deliberately not treated as a
/// Kani gate: the ordinary lane does not evaluate it, so such an item would be
/// present in the test binaries and must keep its mutants.
fn is_kani_only(attrs: &[Attribute]) -> bool {
    attrs.iter().any(is_kani_gate)
}

/// True when `attr` is a `cfg` whose predicate cannot hold without `kani`.
///
/// The whole predicate is evaluated rather than scanned for a `kani` token, so
/// compound forms classify by what they actually permit:
///
/// - `cfg(kani)`, `cfg(all(kani))` and `cfg(all(kani, unix))` are Kani gates,
///   because every conjunct must hold;
/// - `cfg(any(kani, unix))` is **not**, because `unix` alone can satisfy it and
///   the item then compiles in the ordinary lane;
/// - `cfg(not(kani))` is not, which is what makes it the production
///   counterpart.
///
/// Treating every predicate that merely mentions `kani` as a gate would
/// misclassify `cfg(any(kani, unix))` as proof-only and, worse, hide a genuine
/// production counterpart from the counterpart check.
fn is_kani_gate(attr: &Attribute) -> bool {
    let Meta::List(list) = &attr.meta else {
        return false;
    };
    if !list.path.is_ident("cfg") {
        return false;
    }
    let Ok(predicate) = syn::parse2::<Meta>(list.tokens.clone()) else {
        // An unparsable predicate is not something this model can classify, so
        // it is treated as no gate rather than being guessed at.
        return false;
    };
    predicate_requires_kani(&predicate)
}

/// True when the cfg `predicate` cannot hold unless `kani` holds.
///
/// The recursion implements the two combinators that decide the question:
///
/// - `all(..)` requires Kani when **any** conjunct does, because every conjunct
///   must hold.
/// - `any(..)` requires Kani only when **every** disjunct does, because one
///   satisfiable disjunct is enough.
///
/// A negation is never a gate: `not(kani)` holds precisely when Kani is absent.
/// No argument list arrives here, so `parse` is infallible in practice.
fn predicate_requires_kani(predicate: &Meta) -> bool {
    match predicate {
        Meta::Path(path) => path.is_ident("kani"),
        Meta::List(list) => {
            let operands = nested_metas(list);
            let is_all = list.path.is_ident("all");
            let is_any = list.path.is_ident("any");
            (is_all && operands.iter().any(predicate_requires_kani))
                || (is_any && !operands.is_empty() && operands.iter().all(predicate_requires_kani))
        }
        Meta::NameValue(_) => false,
    }
}

/// Parse a cfg combinator's operands.
///
/// Returns an empty list rather than failing: a caller that cannot read the
/// operands treats the combinator as not requiring Kani, which is the
/// conservative answer for a gate whose contents are unknown.
fn nested_metas(list: &syn::MetaList) -> Vec<Meta> {
    syn::parse::Parser::parse2(
        syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
        list.tokens.clone(),
    )
    .map_or_else(|_| Vec::new(), |parsed| parsed.into_iter().collect())
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
#[derive(Debug, Clone, Copy, Default)]
struct Scope {
    /// Some enclosing scope excludes its contents from mutation.
    excluded: bool,
    /// Some enclosing scope compiles only under `cfg(kani)`.
    kani_only: bool,
}

impl Scope {
    /// Return the scope inside `attrs`, inheriting `self`.
    fn nested(self, attrs: &[Attribute]) -> Self {
        Self {
            excluded: self.excluded || is_excluded(attrs),
            kani_only: self.kani_only || is_kani_only(attrs),
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
    collect_items(&parsed.items, file_scope, &mut by_name);
    Ok(by_name)
}

/// Fold a list of items into `by_name`, recursing through inline modules.
fn collect_items(items: &[Item], scope: Scope, by_name: &mut BTreeMap<String, Selection>) {
    for item in items {
        match item {
            Item::Impl(block) => accumulate_impl(block, scope, by_name),
            Item::Mod(module) => accumulate_module(module, scope, by_name),
            _ => {}
        }
    }
}

/// Fold one inline module's items into `by_name`, inheriting its decisions.
fn accumulate_module(module: &ItemMod, scope: Scope, by_name: &mut BTreeMap<String, Selection>) {
    // `mod foo;` has no inline content: its items live in another file, which
    // this reader does not follow. `cargo-mutants` does, so a source with such
    // a declaration is outside what this model can say anything about.
    let Some((_, items)) = &module.content else {
        return;
    };
    collect_items(items, scope.nested(&module.attrs), by_name);
}

/// Fold one `impl` block's methods into `by_name`.
///
/// An exclusion or a Kani gate on this block joins whatever the file and
/// modules already decided, so a method inherits them all.
fn accumulate_impl(block: &ItemImpl, scope: Scope, by_name: &mut BTreeMap<String, Selection>) {
    let block_scope = scope.nested(&block.attrs);
    for item in &block.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };
        let implementation = Implementation {
            name: method.sig.ident.to_string(),
            skipped: block_scope.excluded || is_excluded(&method.attrs) || is_ineligible(method),
            kani_only: block_scope.kani_only || is_kani_only(&method.attrs),
        };
        let entry = by_name.entry(implementation.name).or_default();
        if implementation.skipped {
            entry.skipped += 1;
            if !implementation.kani_only {
                entry.skipped_outside_kani += 1;
            }
        } else {
            entry.selected += 1;
            if !implementation.kani_only {
                entry.selected_production += 1;
            }
        }
    }
}

#[cfg(test)]
mod scope_tests;
#[cfg(test)]
mod tests;
