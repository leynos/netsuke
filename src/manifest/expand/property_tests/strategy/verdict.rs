//! A static entry whose condition verdict is constructed, not hoped for.

use super::Bindings;
use super::condition::{CmpOp, Condition, VAR_NAMES};
use super::generate::fields_strategy;
use super::layout::{ControlLayout, ControlOrder};
use super::spec::BodySpec;
use crate::manifest::ManifestValue;
use proptest::prelude::*;
use proptest::sample::Index;

/// Generate one static body whose condition verdict is fixed.
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

/// Pick a constant making `operator` return `verdict_holds` against zero.
fn verdict_constant(operator: CmpOp, verdict_holds: bool) -> i64 {
    // For a left operand of zero, one of -1, 0, and 1 always produces the
    // wanted verdict for any operator; searching the three keeps this a
    // property of `apply` rather than a second table of operator meanings.
    [-1_i64, 0, 1]
        .into_iter()
        .find(|constant| operator.apply(0, *constant) == verdict_holds)
        .unwrap_or(0)
}
