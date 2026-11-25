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
//! plausible simplification of it. Three details matter, and all were measured
//! against the pinned tool:
//!
//! - `#[mutants::skip]` is matched on its attribute **path**, so the
//!   parenthesized spellings `#[mutants::skip()]` and `#[mutants::skip(..)]`
//!   are honoured too; the `mutants` crate's attribute ignores its arguments.
//! - Inside a `cfg_attr` the skip is honoured only when the condition is a
//!   **bare path**. The tool parses `cfg_attr` arguments as nested metadata, so
//!   `cfg_attr(any(), mutants::skip)` is rejected outright and the skip is
//!   silently dropped.
//! - An *enclosing* exclusion removes everything inside it. Its walker returns
//!   early from `visit_file` and from `visit_item_impl` before it reaches the
//!   methods, so an exclusion on the `impl` block — or on the file, as an inner
//!   attribute — suppresses every method's mutants even though none of them
//!   carries an attribute of its own.
//! - `#[mutants::skip]` is not the only exclusion the walker applies. Its
//!   `attrs_excluded` also matches a bare `#[cfg(test)]`. The test gate is
//!   narrow: only a `test` at the cfg's *top level* counts, so a `test` nested
//!   in `all(..)` or `any(..)` is still mutated. An excluded item produces no
//!   mutants, so counting it as selected would overstate what the ordinary lane
//!   judges.
//!
//! Reading only a method's own `#[mutants::skip]` would therefore report a
//! method as selected while cargo-mutants excludes it. [`implementations_by_name`]
//! carries every enclosing decision down to each method so the two cannot
//! disagree.

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
/// Mirrors the rule cargo-mutants applies. Its reader tests the attribute
/// *path*, which `syn` exposes for every metadata form, so
/// `#[mutants::skip]`, `#[mutants::skip()]` and `#[mutants::skip(anything)]`
/// are all honoured — the `mutants` crate's attribute ignores its arguments.
/// The path check therefore comes first and does not require the `Meta::Path`
/// form. Measured against the pinned tool: an `impl` marked
/// `#[mutants::skip()]` produces no mutants for any of its methods.
///
/// The `cfg_attr` arm is narrower. A `cfg_attr` condition written with
/// parentheses — `any()`, `all(kani, test)` — is not a path, and the tool's
/// `parse_nested_meta` rejects the whole attribute, so a skip written inside it
/// is silently dropped. Recognizing only the accepted forms here is what stops
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

/// True when `attrs` exclude the node from mutation altogether.
///
/// cargo-mutants excludes a whole subtree on either of two conditions:
/// `#[mutants::skip]`, or the test-only gate its `attrs_excluded` also checks.
///
/// The test gate is deliberately narrow. The tool reads the `cfg`'s
/// *top-level* conditions, so a bare `#[cfg(test)]` excludes the node while a
/// `test` nested inside `all(..)` or `any(..)` is not seen at all. Measured
/// against cargo-mutants 27.1.0: `#[cfg(test)]` yields no mutant, whereas
/// `#[cfg(all(test, unix))]` and `#[cfg(any(test, feature = "x"))]` both still
/// do. Modelling the broader "any cfg mentioning test" rule would mark mutants
/// as excluded that the tool would report.
fn is_excluded(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(|attr| is_mutants_skip(attr) || is_cfg_test(attr))
}

/// True when `attr` is a `cfg` attribute naming `test` among its conditions.
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

/// Collect every implementation in `source`, grouped by method name.
///
/// # Errors
///
/// Returns an error when `source` does not parse as a Rust file.
pub fn implementations_by_name(source: &str) -> Result<BTreeMap<String, Selection>> {
    let parsed: File = syn::parse_str(source).context("parse Rust source as a file")?;
    // A file-level inner attribute such as `#![mutants::skip]` is on the file
    // itself, and cargo-mutants returns from `visit_file` before walking any
    // item — as it does for a crate-level `#![cfg(test)]`.
    let file_excluded = is_excluded(&parsed.attrs);
    let mut by_name: BTreeMap<String, Selection> = BTreeMap::new();
    for item in &parsed.items {
        let Item::Impl(block) = item else {
            continue;
        };
        accumulate_impl(block, file_excluded, &mut by_name);
    }
    Ok(by_name)
}

/// Fold one `impl` block's methods into `by_name`.
///
/// `enclosing_excluded` carries the file-level decision; an exclusion on this
/// block is added to it, so a method inherits either.
fn accumulate_impl(
    block: &ItemImpl,
    enclosing_excluded: bool,
    by_name: &mut BTreeMap<String, Selection>,
) {
    let block_excluded = enclosing_excluded || is_excluded(&block.attrs);
    for item in &block.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };
        let implementation = Implementation {
            name: method.sig.ident.to_string(),
            skipped: block_excluded || is_excluded(&method.attrs),
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

#[cfg(test)]
mod tests {
    //! Probes for the rules this module models.
    //!
    //! Each case is a source fragment fed to [`implementations_by_name`], so it
    //! states what the reader must decide rather than what the repository
    //! happens to contain. The graph module's own selection is asserted in
    //! `tests/mutation_selection_contract_tests.rs`; these are about the reader.

    use anyhow::{Context, Result, ensure};

    use super::implementations_by_name;

    #[test]
    fn the_reader_ignores_a_skip_that_cargo_mutants_would_drop() -> Result<()> {
        // `cfg_attr(any(), mutants::skip)` is not honoured by cargo-mutants: its
        // reader accepts only a bare-path condition, so a skip written inside a
        // parenthesized one is silently discarded. A contract that counted it as a
        // skip would certify an exclusion the pinned tool does not perform.
        let source = "\
impl Demo {
    #[cfg_attr(any(), mutants::skip)]
    fn parenthesized_condition(&self) -> u32 { 1 }

    #[cfg_attr(test, mutants::skip)]
    fn bare_path_condition(&self) -> u32 { 2 }
}
";
        let by_name = implementations_by_name(source)?;
        let dropped = by_name
            .get("parenthesized_condition")
            .context("the probe must declare `parenthesized_condition`")?;
        ensure!(
            (dropped.selected, dropped.skipped) == (1, 0),
            "a skip under a parenthesized cfg_attr condition must read as selected, \
         because cargo-mutants discards it; got {dropped:?}"
        );

        let honoured = by_name
            .get("bare_path_condition")
            .context("the probe must declare `bare_path_condition`")?;
        ensure!(
            (honoured.selected, honoured.skipped) == (0, 1),
            "a skip under a bare-path cfg_attr condition must read as skipped, \
         because cargo-mutants honours it; got {honoured:?}"
        );
        Ok(())
    }

    #[test]
    fn the_reader_honours_the_parenthesized_skip_spelling() -> Result<()> {
        // cargo-mutants matches the attribute *path*, so `#[mutants::skip()]` and
        // any argument list are honoured; the `mutants` crate's attribute ignores
        // its arguments. A reader that only recognized the bare `Meta::Path` form
        // would miss this spelling and report a skipped item as selected.
        let source = "\
#[mutants::skip()]
impl Demo {
    fn under_parenthesized_skip(&self) -> u32 { 1 }
    fn also_under_parenthesized_skip(&self) -> u32 { 2 }
}
";
        let by_name = implementations_by_name(source)?;
        for name in ["under_parenthesized_skip", "also_under_parenthesized_skip"] {
            let selection = by_name
                .get(name)
                .with_context(|| format!("the probe must declare `{name}`"))?;
            ensure!(
                (selection.selected, selection.skipped) == (0, 1),
                "`{name}` sits in a block marked `#[mutants::skip()]`, which \
             cargo-mutants honours, so it must read as skipped; got {selection:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn the_reader_honours_an_enclosing_impl_skip() -> Result<()> {
        // cargo-mutants returns from `visit_item_impl` before it reaches the
        // methods, so a skip on the block suppresses every method in it even though
        // no method carries an attribute. A reader that inspected only the method's
        // own attributes would call these selected and disagree with the tool it
        // claims to model.
        let source = "\
#[mutants::skip]
impl Demo {
    fn under_skipped_impl(&self) -> u32 { 1 }
}
";
        let by_name = implementations_by_name(source)?;
        let enclosed = by_name
            .get("under_skipped_impl")
            .context("the probe must declare `under_skipped_impl`")?;
        ensure!(
            (enclosed.selected, enclosed.skipped) == (0, 1),
            "a method inside a skipped impl block must read as skipped, because \
         cargo-mutants never visits it; got {enclosed:?}"
        );
        Ok(())
    }

    #[test]
    fn the_reader_excludes_bare_cfg_test_like_the_tool_does() -> Result<()> {
        // `#[mutants::skip]` is not the only exclusion cargo-mutants applies: its
        // `attrs_excluded` also matches a `cfg` whose own condition is exactly
        // `test`, at both `impl` and method scope. Counting such an item as selected
        // would claim the ordinary lane judges code the tool never mutates.
        //
        // The rule is deliberately narrower than "any cfg mentioning test". The
        // tool reads the cfg's *top-level* conditions, so a `test` nested inside
        // `all(..)` or `any(..)` is not seen. Measured against cargo-mutants
        // 27.1.0: `#[cfg(test)]` yields no mutant, while `#[cfg(all(test, unix))]`
        // and `#[cfg(any(test, feature = "x"))]` both still do. Modelling the
        // broader rule would mark mutants as excluded that the tool would report.
        let source = "\
#[cfg(test)]
impl TestOnly {
    fn under_cfg_test_impl(&self) -> u32 { 1 }
}

impl Demo {
    fn plain(&self) -> u32 { 2 }

    #[cfg(test)]
    fn under_cfg_test_method(&self) -> u32 { 3 }

    #[cfg(all(test, unix))]
    fn under_nested_test_cfg(&self) -> u32 { 4 }
}
";
        let by_name = implementations_by_name(source)?;
        for name in ["under_cfg_test_impl", "under_cfg_test_method"] {
            let selection = by_name
                .get(name)
                .with_context(|| format!("the probe must declare `{name}`"))?;
            ensure!(
                (selection.selected, selection.skipped) == (0, 1),
                "`{name}` carries a bare `#[cfg(test)]` and produces no mutants, so \
             it must read as excluded rather than selected; got {selection:?}"
            );
        }

        for name in ["plain", "under_nested_test_cfg"] {
            let selection = by_name
                .get(name)
                .with_context(|| format!("the probe must declare `{name}`"))?;
            ensure!(
                (selection.selected, selection.skipped) == (1, 0),
                "`{name}` still produces mutants — a `test` nested in `all(..)` is \
             not seen by the tool's top-level scan — so it must stay selected; \
             got {selection:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn the_reader_honours_a_file_level_skip() -> Result<()> {
        // A file-level `#![mutants::skip]` is an inner attribute on the file, and
        // the tool returns from `visit_file` before walking any item. Nothing
        // inside such a file produces mutants.
        let source = "\
#![mutants::skip]

impl Demo {
    fn in_a_skipped_file(&self) -> u32 { 1 }
}
";
        let by_name = implementations_by_name(source)?;
        let enclosed = by_name
            .get("in_a_skipped_file")
            .context("the probe must declare `in_a_skipped_file`")?;
        ensure!(
            (enclosed.selected, enclosed.skipped) == (0, 1),
            "a method in a file carrying an inner mutants::skip must read as \
         skipped, because cargo-mutants never walks it; got {enclosed:?}"
        );
        Ok(())
    }
}
