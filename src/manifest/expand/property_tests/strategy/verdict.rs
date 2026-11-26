//! A static entry whose condition verdict is constructed, not hoped for.
//!
//! The coverage tripwire requires each verdict class to reach a share of
//! draws. Leaving that to a random comparison put one class below the floor,
//! so the verdict is chosen first and a comparison built to satisfy it.

use super::Bindings;
use super::condition::{CmpOp, Condition, VAR_NAMES};
use super::generate::fields_strategy;
use super::layout::{ControlLayout, ControlOrder};
use super::spec::BodySpec;
use crate::manifest::ManifestValue;
use proptest::prelude::*;
use proptest::sample::Index;

/// Generate one static body whose condition verdict is fixed.
///
/// The verdict is constructed rather than drawn: the bindings are chosen so
/// the condition holds or fails, which guarantees each of the four
/// section-and-spelling verdict classes per draw. Hoping a random comparison
/// fails often enough put `actions-failed-expression` below the once-per-draw
/// floor.
pub(super) fn static_verdict_body(verdict_holds: bool) -> impl Strategy<Value = BodySpec> {
    (
        fields_strategy(),
        Condition::strategy(),
        (any::<Index>(), any::<Index>()),
        ControlOrder::strategy(),
    )
        .prop_flat_map(
            move |(fields, shape, (control_position, vars_position), order)| {
                let (condition, bindings) = verdict_condition(shape, verdict_holds);
                let vars = condition_vars(condition, &bindings);
                any::<bool>()
                    .prop_map(move |drawn| drawn && condition.has_template_form())
                    .prop_map(move |template| BodySpec {
                        fields: fields.clone(),
                        vars: Some(vars.clone()),
                        vars_position,
                        foreach: None,
                        when: Some((condition, template)),
                        control_layout: ControlLayout::drawn(control_position),
                        second_control_layout: ControlLayout::drawn(control_position),
                        control_order: order,
                        bindings: bindings.clone(),
                    })
            },
        )
}

/// Build the authored `vars` entries the condition's free variable needs.
pub(super) fn condition_vars(
    condition: Condition,
    bindings: &Bindings,
) -> Vec<(String, ManifestValue)> {
    condition
        .variable()
        .and_then(|name| {
            VAR_NAMES
                .iter()
                .position(|candidate| *candidate == name)
                .map(|slot| (name, slot))
        })
        .map_or_else(Vec::new, |(name, slot)| {
            vec![(
                name.to_owned(),
                Bindings::binding_for(condition, slot, bindings),
            )]
        })
}

/// Build a condition and the bindings that make it hold or fail.
///
/// The shape is drawn from the same closed set the rest of the suite uses, so
/// the property keeps generating literals, flags, negations, truthiness tests,
/// and comparisons; only the bindings are constructed, to force the requested
/// verdict. Iteration shapes are replaced because a static entry cannot carry
/// them.
fn verdict_condition(shape: Condition, verdict_holds: bool) -> (Condition, Bindings) {
    shape_with_verdict(shape, verdict_holds)
}

/// Force one drawn shape to the requested verdict.
fn shape_with_verdict(shape: Condition, verdict_holds: bool) -> (Condition, Bindings) {
    let literal = |holds: bool| (Condition::Literal(holds), Bindings::with_count(0, 0));
    match shape {
        // A static entry cannot read either iteration binding, and a literal
        // is simply replaced.
        Condition::Literal(_) | Condition::IndexCmp(..) | Condition::ItemCmp(..) => {
            literal(verdict_holds)
        }
        Condition::Flag(slot) => (shape, Bindings::with_flag(slot, verdict_holds)),
        Condition::NotFlag(slot) => (shape, Bindings::with_flag(slot, !verdict_holds)),
        Condition::Count(slot) => (shape, Bindings::with_count(slot, i64::from(verdict_holds))),
        Condition::VarCmp(slot, operator, _) => (
            Condition::VarCmp(slot, operator, verdict_constant(operator, verdict_holds)),
            Bindings::with_count(slot, 0),
        ),
    }
}

/// Pick a constant that makes `operator` return `verdict_holds` for a left
/// operand of zero.
fn verdict_constant(operator: CmpOp, verdict_holds: bool) -> i64 {
    // For a left operand of zero, one of -1, 0, and 1 always produces the
    // wanted verdict for any operator; searching the three keeps this a
    // property of `apply` rather than a second table of operator meanings.
    [-1_i64, 0, 1]
        .into_iter()
        .find(|constant| operator.apply(0, *constant) == verdict_holds)
        .unwrap_or(0)
}
