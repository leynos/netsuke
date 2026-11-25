//! Decide whether a `cfg` predicate can hold without `kani`.
//!
//! This is the classification behind `super::is_kani_only`. It is separate
//! because the question has real boolean structure: negation inverts the
//! combinator, not just the atom, so `not(not(kani))` is a Kani gate while
//! `not(kani)` is its production counterpart. A flat token scan cannot tell
//! those apart, and getting it wrong either hides a proof-only implementation
//! from the counterpart check or claims production code that never compiles in
//! the ordinary lane.

use syn::{Attribute, Meta};

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
pub(super) fn is_kani_gate(attr: &Attribute) -> bool {
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
/// The recursion carries a `negated` polarity rather than treating `not(..)` as
/// an opaque no-gate. That matters because negation inverts the combinator, not
/// just the atom: `not(not(kani))` is `kani` and *is* a gate, while a flat "a
/// negation is never a gate" rule would call it production code and let a
/// missing counterpart pass the check.
///
/// De Morgan drives the two combinators, in both polarities:
///
/// - positive `all(..)` requires Kani when **any** conjunct does, because every
///   conjunct must hold; negated, it becomes a disjunction of negations and
///   requires Kani only when **every** negated conjunct does.
/// - positive `any(..)` requires Kani only when **every** disjunct does;
///   negated, it becomes a conjunction of negations and requires Kani when
///   **any** negated disjunct does.
///
/// Any other predicate — `not(kani)`, `unix`, `feature = "x"` — is not a gate:
/// each can hold while Kani is absent.
fn predicate_requires_kani(predicate: &Meta) -> bool {
    requires_kani_with(predicate, false)
}

/// True when `predicate` under `negated` polarity cannot hold without `kani`.
fn requires_kani_with(predicate: &Meta, negated: bool) -> bool {
    match predicate {
        Meta::Path(path) => path.is_ident("kani") != negated,
        Meta::List(list) => {
            let operands = nested_metas(list);
            let require_every = |inner_negated: bool| {
                !operands.is_empty()
                    && operands
                        .iter()
                        .all(|operand| requires_kani_with(operand, inner_negated))
            };
            let require_any = |inner_negated: bool| {
                operands
                    .iter()
                    .any(|operand| requires_kani_with(operand, inner_negated))
            };
            match (list.path.is_ident("all"), list.path.is_ident("any")) {
                // `not(all(..))` is a disjunction of negated conjuncts.
                (true, _) if negated => require_every(true),
                (true, _) => require_any(false),
                // `not(any(..))` is a conjunction of negated disjuncts.
                (_, true) if negated => require_any(true),
                (_, true) => require_every(false),
                // `not(..)` flips the polarity of its single operand.
                _ if list.path.is_ident("not") => operands
                    .first()
                    .is_some_and(|operand| requires_kani_with(operand, !negated)),
                _ => false,
            }
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
