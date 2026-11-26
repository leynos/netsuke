//! Identity of an `impl` block: what makes two implementations the same one.
//!
//! cargo-mutants names an implementation by its rendered self type, prefixed by
//! the lexical module path and, for a trait implementation, wrapped as
//! `<impl Trait for Type>`. Anything shorter merges implementations that the
//! tool keeps apart, and a skip on one then answers a check about the other.
//!
//! Three distinctions matter and all are handled here: the module path, the
//! generic arguments of the self type, and the implemented trait.

use quote::ToTokens;
use syn::ItemImpl;

/// True when `block` implements `Default`.
pub(super) fn is_default_impl(block: &ItemImpl) -> bool {
    block
        .trait_
        .as_ref()
        .and_then(|(_, path, _)| path.segments.last())
        .is_some_and(|segment| segment.ident == "Default")
}

/// The full name an `impl` block declares itself for.
///
/// Rendered from the type's own tokens rather than from its last segment, so
/// two impls of the same nominal type stay distinct however they differ:
/// `other::BuildGraph` from `BuildGraph`, `Demo<u8>` from `Demo<u16>`, and a
/// reference from a tuple. The pinned tool puts this same rendered text in its
/// implementation namespace, so shortening it would merge implementations and
/// let one's skip answer a check about another's.
fn self_ty_name(block: &ItemImpl) -> String {
    compact(&block.self_ty)
}

/// Render `node` as the compact text cargo-mutants prints.
///
/// `TokenStream`'s own `Display` separates every token with a space, so
/// `other::BuildGraph` would become `other :: BuildGraph`. Collapsing the
/// spacing around path separators and angle brackets keeps the key readable and
/// stable — it is an identity, never asserted on for its exact spelling.
fn compact(node: &impl ToTokens) -> String {
    node.to_token_stream()
        .to_string()
        .replace(" :: ", "::")
        .replace(" < ", "<")
        .replace(" > ", ">")
        .replace("< ", "<")
        .replace(" >", ">")
}

/// The trait an `impl` block implements, when it implements one.
///
/// Included in the owner key because cargo-mutants gives a trait implementation
/// its own namespace: `<impl OtherTrait for Demo>` and an inherent `impl Demo`
/// can each declare `paired`, and they are different implementations.
fn trait_name(block: &ItemImpl) -> Option<String> {
    block.trait_.as_ref().map(|(_, path, _)| compact(path))
}

/// The owner key for `block`, given the lexical module path it sits in.
///
/// The implementation component is built first and the module path is
/// prepended to it, which is the order cargo-mutants prints: `m::Demo` inherent
/// is `m::Demo`, and `impl Trait for Demo` inside `m` is
/// `m::<impl Trait for Demo>`. Wrapping the *already qualified* type instead
/// would make `m::<impl Trait for Demo>` and `<impl Trait for m::Demo>`
/// indistinguishable, and those are different implementations.
pub(super) fn implementation_owner(block: &ItemImpl, modules: &[String]) -> String {
    let component = match trait_name(block) {
        Some(trait_path) => format!("<impl {trait_path} for {}>", self_ty_name(block)),
        None => self_ty_name(block),
    };
    let mut path = modules.to_vec();
    path.push(component);
    path.join("::")
}
