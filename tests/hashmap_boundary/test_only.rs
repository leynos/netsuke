//! Deciding whether an item is compiled only for tests.
//!
//! The emitter's boundary contract rests on stripping test-only code, and Rust
//! expresses that in two places: an item carrying `#[cfg(test)]`, and a file
//! named by a `#[cfg(test)] mod` declaration. Both questions reduce to one:
//! does this `cfg` predicate *imply* the test configuration?
//!
//! Reading the predicate as text would answer a different question. `test`,
//! `all(test, unix)`, `any(test, unix)`, and `not(test)` all spell the word, and
//! only two of them are test-only: `any(test, unix)` compiles on Unix, and
//! `not(test)` compiles everywhere else. The predicate is therefore parsed as
//! the tree it is, and the answer is derived from what the tree requires.
//!
//! The module is separate from `scan.rs` because the question has two
//! independent callers — the declaration scan and the use scan in `bindings.rs` —
//! and because `scan.rs` would otherwise pass the repository's line cap. Its
//! answer is the same whichever caller asks, which is the point of one home.

use syn::{Attribute, File, Item, Meta, Path};

/// Return whether `file` declares a child module called `name` only for tests.
///
/// The boundary walk calls this with a directory's own `mod.rs` and each of its
/// children: the child's *contents* carry no attribute, so the parent is the
/// only place the answer exists. A declaration the file does not carry, or one
/// with no `cfg`, answers `false` — an undeclared child cannot be found here,
/// and the walk has already restricted itself to files this repository's layout
/// contract makes declarable.
pub(super) fn declares_test_only_module(file: &File, name: &str) -> bool {
    file.items.iter().any(|item| match item {
        Item::Mod(module) => module.ident == name && is_test_only(item),
        _ => false,
    })
}

/// Return whether `item` is compiled only for tests.
///
/// Both scans share this rather than each reading attributes, so a test-only
/// construct cannot be excused by one half of the contract and reported by the
/// other. An item kind not listed here carries no `cfg` position this crate
/// reasons about, and answers `false` rather than being assumed test-only.
pub(super) fn is_test_only(item: &Item) -> bool {
    let attributes = match item {
        Item::Mod(named) => &named.attrs,
        Item::Use(named) => &named.attrs,
        Item::Struct(named) => &named.attrs,
        Item::Enum(named) => &named.attrs,
        Item::Fn(named) => &named.attrs,
        Item::Const(named) => &named.attrs,
        Item::Static(named) => &named.attrs,
        Item::Impl(named) => &named.attrs,
        Item::Type(named) => &named.attrs,
        Item::Trait(named) => &named.attrs,
        _ => return false,
    };
    attributes.iter().any(implies_test)
}

/// Return whether one `cfg` attribute holds only in the test configuration.
///
/// The predicate is read as a tree rather than as text, and the question asked
/// is whether it *implies* `test`:
///
/// - `test` does.
/// - `all(a, b)` does when either conjunct does, since both must hold.
/// - `any(a, b)` does only when both disjuncts do, since either may hold.
/// - `not(..)` never does, and neither does any other predicate.
///
/// The distinction matters at the call site: `cfg(all(test, unix))` is never
/// compiled in a release build, while `cfg(any(test, unix))` is.
fn implies_test(attribute: &Attribute) -> bool {
    if !names_cfg(attribute.path()) {
        return false;
    }
    let Ok(meta) = attribute.parse_args::<Meta>() else {
        return false;
    };
    meta_implies_test(&meta)
}

/// Return whether an attribute path is the `cfg` macro.
fn names_cfg(path: &Path) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == "cfg")
}

/// Return whether one parsed `cfg` predicate implies the test configuration.
fn meta_implies_test(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => is_test_ident(path),
        Meta::List(list) => list
            .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
            .is_ok_and(|operands| predicate_implies_test(&list.path, &operands)),
        Meta::NameValue(_) => false,
    }
}

/// Return whether a combinator's operands imply the test configuration.
///
/// `all` needs one conjunct to imply `test`; `any` needs every disjunct to.
/// Anything else — `not(...)`, `target_os = "..."` — implies nothing.
fn predicate_implies_test(
    combinator: &Path,
    operands: &syn::punctuated::Punctuated<Meta, syn::Token![,]>,
) -> bool {
    let Some(name) = combinator.segments.last() else {
        return false;
    };
    let verdicts: Vec<bool> = operands.iter().map(meta_implies_test).collect();
    match name.ident.to_string().as_str() {
        "all" => verdicts.into_iter().any(|verdict| verdict),
        "any" => verdicts.into_iter().all(|verdict| verdict),
        _ => false,
    }
}

/// Return whether a bare predicate path is the `test` configuration.
fn is_test_ident(path: &Path) -> bool {
    path.segments.len() == 1
        && path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "test")
}
