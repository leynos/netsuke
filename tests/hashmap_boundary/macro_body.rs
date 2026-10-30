//! Reading a macro body as the expressions it mostly is.
//!
//! `syn` visits a [`Macro`] node as tokens, never as a syntax tree, so a name
//! written inside `writeln!(..)` is invisible to a visitor that implements only
//! the item and expression hooks. Two contracts in this crate depend on seeing
//! those names: `scan.rs` asks whether a macro body names an unordered
//! collection, and `bindings.rs` asks whether it reads an exempt field. Both
//! questions need the same two operations, which are stated here once rather
//! than twice with the chance to drift.
//!
//! Most macros in this tree take a comma-separated expression list, and that
//! parses exactly. Some have a grammar of their own — `matches!` takes a
//! pattern after its subject, `vec![a; n]` takes a repeat count — so a body
//! that does not parse falls back to asking whether it *spells* the name at
//! all. The fallback errs towards reporting: it cannot miss a mention, and a
//! spelling that is not a real use is visible in the failure message and
//! dismissed by a human. A guard should fail in the direction a human can see.

use syn::{
    Expr, Macro, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

/// A macro body's arguments, read as the expression list most macros take.
struct Arguments(Punctuated<Expr, Token![,]>);

impl Parse for Arguments {
    /// Parse a comma-separated list that may also be empty.
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Punctuated::parse_terminated(input).map(Self)
    }
}

/// Return the expressions a macro body holds, or `None` if it does not parse.
///
/// `None` means the macro has a grammar of its own rather than that the body is
/// empty: an empty body parses, as zero arguments. The distinction matters to
/// the caller, which must treat the two cases differently — nothing to descend
/// into, against an unreadable body that [`spells`] has to judge.
#[must_use]
pub(super) fn arguments(node: &Macro) -> Option<Vec<Expr>> {
    syn::parse2::<Arguments>(node.tokens.clone())
        .ok()
        .map(|parsed| parsed.0.into_iter().collect())
}

/// Return whether a macro body's tokens spell `name` anywhere.
///
/// The body is rendered with token boundaries intact — `proc_macro2` separates
/// adjacent tokens with a space — and split into identifier-shaped words, so
/// `stages.staged_sidecars` matches and `staged_sidecars_extra` does not.
/// Comparing whole words rather than substrings keeps this to the same
/// exactness the syntax-tree rules use.
#[must_use]
pub(super) fn spells(node: &Macro, name: &str) -> bool {
    node.tokens
        .to_string()
        .split(|character: char| !(character.is_alphanumeric() || character == '_'))
        .any(|word| word == name)
}
