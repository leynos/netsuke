//! Decide whether a `cfg` predicate can hold without `kani`.
//!
//! This is the classification behind `super::is_kani_only`, which the
//! counterpart check relies on: an implementation that cannot compile without
//! `kani` is proof-only, so it must not be counted as a production counterpart.
//!
//! The question is a *satisfiability* one: is there an assignment of the other
//! cfg atoms, with `kani` false, under which the predicate holds? Three-valued
//! answers are the only honest kind here:
//!
//! - [`Verdict::Production`] — such an assignment exists.
//! - [`Verdict::KaniOnly`] — none does.
//! - [`Verdict::Undecidable`] — the model cannot decide, so nothing may be
//!   certified.
//!
//! The third value is what makes this a contract rather than a guess. Answering
//! `Production` for a form the model cannot read would certify a counterpart
//! that may not exist, which is the defect the surrounding test exists to
//! catch; those forms are surfaced and the caller turns them into a failure.
//!
//! The evaluation is exact within a bound rather than a heuristic. It collects
//! the distinct atoms the predicate mentions, enumerates their assignments with
//! `kani` pinned to false, and evaluates the predicate under each. That is what
//! makes the awkward cases come out right:
//!
//! - `any(kani, unix)` — satisfied by `unix = true`, so production code;
//! - `not(unix)` and `not(any(kani, unix))` — satisfied by `unix = false`;
//! - `not(not(kani))` — satisfied by nothing, since it is `kani`;
//! - `any(kani, all(unix, not(unix)))` — `unix` and `not(unix)` cannot both
//!   hold, so no assignment satisfies it and it is a Kani gate;
//! - `cfg(any(kani, unix))` on an `impl` around
//!   `cfg(any(kani, not(unix)))` on its method — the conjunction needs `unix`
//!   and its negation at once, so the pair is a gate even though each half is
//!   not. Assignments are enumerated over the *whole* combined predicate, which
//!   is why nested scopes are conjoined before evaluation rather than combined
//!   as separate verdicts.
//!
//! A heuristic pair-of-truth-values model was tried first and got the last two
//! cases wrong; enumerating assignments removes the class of error rather than
//! adding a case for it.

use std::collections::BTreeSet;

use syn::ext::IdentExt;
use syn::{Attribute, Expr, Lit, Meta};

/// The most distinct atoms the enumeration will span.
///
/// Two to the power of this bounds the work. Exceeding it is reported as
/// undecidable rather than truncated, because a truncated enumeration would
/// answer `KaniOnly` for a predicate it merely failed to satisfy.
const MAX_ATOMS: usize = 12;

/// What the model can say about one condition.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Verdict {
    /// Cannot hold with `kani` false: proof-only code.
    KaniOnly,
    /// Can hold with `kani` false: ordinary code.
    #[default]
    Production,
    /// The model cannot decide, so nothing may be certified either way.
    Undecidable,
}

/// The predicates a set of attributes imposes, or `None` if one is unreadable.
///
/// A `cfg_attr` is not modelled: it expands to whatever it names, and this
/// model does not follow that expansion. Any `cfg_attr` therefore makes the
/// whole set undecidable rather than being skipped, because skipping one could
/// hide a condition that removes the item from the ordinary build.
pub fn predicates(attrs: &[Attribute]) -> Option<Vec<Meta>> {
    let mut found = Vec::new();
    for attr in attrs {
        let Meta::List(list) = &attr.meta else {
            continue;
        };
        if is_attr_named(&list.path, "cfg_attr") {
            return None;
        }
        if !is_attr_named(&list.path, "cfg") {
            continue;
        }
        // `cfg(true)` and `cfg(false)` are literal operands, which `syn::Meta`
        // does not parse. Reading the failure as "no constraint" would call
        // `cfg(any(true))` proof-only by accident, so it is surfaced instead.
        found.push(syn::parse2::<Meta>(list.tokens.clone()).ok()?);
    }
    Some(found)
}

/// The verdict for the conjunction of `predicates`.
///
/// An empty list constrains nothing and is ordinary code.
#[must_use]
pub fn verdict(predicates: &[Meta]) -> Verdict {
    let Some(atoms) = atoms_of_all(predicates) else {
        return Verdict::Undecidable;
    };
    if atoms.iter().any(|atom| is_built_in_value_atom(atom)) {
        return Verdict::Undecidable;
    }
    if atoms.len() > MAX_ATOMS {
        return Verdict::Undecidable;
    }
    let names: Vec<String> = atoms.into_iter().collect();
    // `kani` is pinned to false, so only the other atoms are enumerated.
    for assignment in 0..(1u32 << names.len()) {
        let values: Vec<bool> = (0..names.len())
            .map(|bit| assignment & (1 << bit) != 0)
            .collect();
        let mut satisfiable = Some(true);
        for predicate in predicates {
            match evaluate(predicate, &names, &values) {
                Some(value) => satisfiable = satisfiable.map(|acc| acc && value),
                None => return Verdict::Undecidable,
            }
        }
        if satisfiable == Some(true) {
            return Verdict::Production;
        }
    }
    Verdict::KaniOnly
}

/// Evaluate `predicate` under one assignment of the enumerated atoms.
///
/// Returns `None` for a form the model does not read, which the caller turns
/// into [`Verdict::Undecidable`].
fn evaluate(predicate: &Meta, names: &[String], assignment: &[bool]) -> Option<bool> {
    match predicate {
        // `kani` is false throughout, which is the whole question.
        Meta::Path(path) => lookup(path, names, assignment).or(Some(false)),
        Meta::List(list) => {
            let operands = nested_metas(list)?;
            if list.path.is_ident("all") {
                operands
                    .iter()
                    .map(|operand| evaluate(operand, names, assignment))
                    .try_fold(true, |acc, operand| operand.map(|held| acc && held))
            } else if list.path.is_ident("any") {
                operands
                    .iter()
                    .map(|operand| evaluate(operand, names, assignment))
                    .try_fold(false, |acc, operand| operand.map(|held| acc || held))
            } else if list.path.is_ident("not") {
                operands
                    .first()
                    .and_then(|operand| evaluate(operand, names, assignment))
                    .map(|value| !value)
            } else {
                None
            }
        }
        Meta::NameValue(pair) => lookup_key(&name_value_key(pair)?, names, assignment),
    }
}

/// The value of `path` under the assignment, if it is an enumerated atom.
///
/// A path that is not enumerated is `kani`, whose value is false.
fn lookup(path: &syn::Path, names: &[String], assignment: &[bool]) -> Option<bool> {
    lookup_key(&path_key(path), names, assignment)
}

/// The value of `key` under the assignment, if it is an enumerated atom.
///
/// Shared by both atom shapes so collection and evaluation cannot disagree
/// about what identifies an atom: `collect_atoms` stores a `name = "value"`
/// predicate under [`name_value_key`], and looking it up by bare name would
/// find nothing and report the predicate as undecidable.
fn lookup_key(key: &str, names: &[String], assignment: &[bool]) -> Option<bool> {
    names
        .iter()
        .position(|name| name == key)
        .and_then(|index| assignment.get(index).copied())
}

/// Collect the distinct atom keys across `predicates`.
///
/// Returns `None` for a form whose identity the model cannot key soundly, such
/// as a `feature`-style predicate with a non-literal value.
fn atoms_of_all(predicates: &[Meta]) -> Option<BTreeSet<String>> {
    let mut atoms = BTreeSet::new();
    for predicate in predicates {
        collect_atoms(predicate, &mut atoms)?;
    }
    Some(atoms)
}

/// True when `atom` is a `name = "value"` pair on a built-in option.
///
/// Rust assigns the built-in options from closed, version-dependent domains:
/// `target_endian` is `"little"` or `"big"`, `target_os` is one of a list that
/// grows with each release, and a target may carry several `target_family`
/// values at once. This model cannot know those domains. Enumerating such an
/// atom as free would let it certify code that cannot compile; enumerating it as
/// constrained would need the domain it does not have. Either way the answer
/// would be a guess, so the predicate is declined instead.
///
/// `feature` is deliberately exempt: it is Cargo's key, its value set is open,
/// and several are enabled at once, so a feature atom really is free.
fn is_built_in_value_atom(atom: &str) -> bool {
    let Some((key, _)) = atom.split_once('=') else {
        return false;
    };
    key == "target_arch" || key == "target_os" || key == "panic" || key.starts_with("target_")
}

/// Add every free atom `predicate` mentions to `atoms`.
fn collect_atoms(predicate: &Meta, atoms: &mut BTreeSet<String>) -> Option<()> {
    match predicate {
        Meta::Path(path) => {
            if !is_kani_path(path) {
                atoms.insert(path_key(path));
            }
            Some(())
        }
        Meta::List(list) => {
            if !is_combinator(&list.path) {
                return None;
            }
            for operand in nested_metas(list)? {
                collect_atoms(&operand, atoms)?;
            }
            Some(())
        }
        Meta::NameValue(named) => {
            atoms.insert(name_value_key(named)?);
            Some(())
        }
    }
}

/// True when `path` is one of the three cfg combinators this model reads.
///
/// Split out of the caller because the repository's lint caps a condition at
/// two `||` branches, and naming the set says more than a compound negation.
fn is_combinator(path: &syn::Path) -> bool {
    path.is_ident("all") || path.is_ident("any") || path.is_ident("not")
}

/// The key for a bare atom: its path as written.
fn path_key(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.unraw().to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// True when `path` is the attribute named `name`, in any spelling.
///
/// `#[r#cfg(kani)]` is the same attribute as `#[cfg(kani)]` to rustc: the raw
/// marker is spelling, not identity. Comparing without normalizing would let a
/// raw-spelled `cfg` bypass the model entirely and be read as unconstrained.
fn is_attr_named(path: &syn::Path, name: &str) -> bool {
    path.get_ident().is_some_and(|ident| ident.unraw() == name)
}

/// True when `path` names the `kani` atom in any spelling.
fn is_kani_path(path: &syn::Path) -> bool {
    path.get_ident()
        .is_some_and(|ident| ident.unraw() == "kani")
}

/// The key for `name = value`, which must be a string or Boolean literal.
///
/// Including the literal keeps `feature = "a"` and `feature = "b"` distinct
/// atoms; keying only on the name would make two unrelated conditions look
/// correlated.
fn name_value_key(named: &syn::MetaNameValue) -> Option<String> {
    let name = path_key(&named.path);
    match &named.value {
        Expr::Lit(literal) => match &literal.lit {
            Lit::Str(text) => Some(format!("{name}={:?}", text.value())),
            Lit::Bool(value) => Some(format!("{name}={}", value.value)),
            _ => None,
        },
        _ => None,
    }
}

/// Parse a cfg combinator's operands.
///
/// Returns `None` when the operands cannot be read as metadata. Each operand is
/// a predicate, and a literal such as `any(true)` is not one, so the caller
/// must not treat the failure as an empty list: an empty `any()` is false,
/// whereas an unreadable `any(true)` is always true.
fn nested_metas(list: &syn::MetaList) -> Option<Vec<Meta>> {
    syn::parse::Parser::parse2(
        syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
        list.tokens.clone(),
    )
    .ok()
    .map(|parsed| parsed.into_iter().collect())
}
