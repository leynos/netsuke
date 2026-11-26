//! Decide whether a `cfg` predicate can hold without `kani`.
//!
//! This is the classification behind `super::is_kani_only`, which the
//! counterpart check relies on: an implementation that cannot compile without
//! `kani` is proof-only, so it must not be counted as a production counterpart.
//!
//! The question is a *satisfiability* one — "is there any assignment of the
//! other cfg atoms, with `kani` false, that makes this predicate hold?" — and it
//! cannot be answered by scanning for the `kani` token, nor by tracking a single
//! negation polarity. Two counter-examples that a polarity model gets wrong:
//!
//! - `not(unix)` holds whenever `unix` is false, so it is satisfiable without
//!   `kani` and is *not* a gate.
//! - `not(any(kani, unix))` is `all(not(kani), not(unix))`, which holds when
//!   both atoms are false, so it is not a gate either.
//!
//! The model below therefore evaluates each subexpression to the pair of truth
//! values it *can* take — can it be true, and can it be false — rather than to a
//! single value. `kani` is special-cased to "false only", every other atom is
//! unconstrained, and negation swaps the pair. A predicate is a gate exactly
//! when it cannot be true.

use syn::{Attribute, Meta};

/// The truth values a predicate can take when `kani` is false.
///
/// Every other cfg atom is treated as unconstrained, which is what makes the
/// answer independent of the host: `unix` may or may not hold, so a predicate
/// mentioning it is satisfiable without `kani` if *any* assignment of that atom
/// works.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Possibility {
    /// Some assignment of the other atoms makes the predicate true.
    can_be_true: bool,
    /// Some assignment of the other atoms makes the predicate false.
    can_be_false: bool,
}

impl Possibility {
    /// A predicate whose value the model cannot constrain.
    const FREE: Self = Self {
        can_be_true: true,
        can_be_false: true,
    };

    /// A predicate that is false in every assignment: `kani` with Kani absent.
    const ALWAYS_FALSE: Self = Self {
        can_be_true: false,
        can_be_false: true,
    };

    /// The pair for `not(self)`.
    fn negated(self) -> Self {
        Self {
            can_be_true: self.can_be_false,
            can_be_false: self.can_be_true,
        }
    }
}

/// True when `attr` is a `cfg` whose predicate cannot hold without `kani`.
///
/// The whole predicate is evaluated rather than scanned for a `kani` token, so
/// compound forms classify by what they actually permit:
///
/// - `cfg(kani)`, `cfg(all(kani))`, `cfg(all(kani, unix))` and
///   `cfg(not(not(kani)))` are gates: no assignment satisfies them with Kani
///   absent;
/// - `cfg(any(kani, unix))`, `cfg(not(kani))`, `cfg(not(unix))` and
///   `cfg(not(any(kani, unix)))` are not, because each can hold while Kani is
///   absent.
///
/// Treating every predicate that merely mentions `kani` as a gate would
/// misclassify the second group as proof-only and, worse, hide a genuine
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
    !possibility(&predicate).can_be_true
}

/// The truth values `predicate` can take when `kani` is false.
fn possibility(predicate: &Meta) -> Possibility {
    match predicate {
        Meta::Path(path) => {
            if path.is_ident("kani") {
                Possibility::ALWAYS_FALSE
            } else {
                Possibility::FREE
            }
        }
        Meta::List(list) => {
            let operands = nested_metas(list);
            let values: Vec<Possibility> = operands.iter().map(possibility).collect();
            if list.path.is_ident("all") {
                // Every conjunct must hold, so the group can be true only if
                // all of them can, and false if any of them can be.
                Possibility {
                    can_be_true: values.iter().all(|value| value.can_be_true),
                    can_be_false: values.iter().any(|value| value.can_be_false),
                }
            } else if list.path.is_ident("any") {
                // One disjunct is enough to make the group true, but all of
                // them must be able to be false for the group to be false.
                Possibility {
                    can_be_true: values.iter().any(|value| value.can_be_true),
                    can_be_false: values.iter().all(|value| value.can_be_false),
                }
            } else if list.path.is_ident("not") {
                values
                    .first()
                    .copied()
                    .map_or(Possibility::FREE, Possibility::negated)
            } else {
                // An unknown combinator. `FREE` is the conservative answer: it
                // keeps the item classified as production rather than claiming
                // a gate the model cannot justify.
                Possibility::FREE
            }
        }
        Meta::NameValue(_) => Possibility::FREE,
    }
}

/// Parse a cfg combinator's operands.
///
/// Returns an empty list rather than failing: a caller that cannot read the
/// operands treats the combinator as unconstrained, which keeps the item
/// classified as production rather than claiming a gate it cannot justify.
fn nested_metas(list: &syn::MetaList) -> Vec<Meta> {
    syn::parse::Parser::parse2(
        syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
        list.tokens.clone(),
    )
    .map_or_else(|_| Vec::new(), |parsed| parsed.into_iter().collect())
}
