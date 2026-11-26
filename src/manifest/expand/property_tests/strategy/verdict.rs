//! A static entry whose condition verdict is constructed, not hoped for.
//!
//! The coverage tripwire requires each verdict class to reach a share of
//! draws. Leaving that to a random comparison put one class below the floor,
//! so the verdict is chosen first and a comparison built to satisfy it.

use super::Bindings;
use super::condition::{CmpOp, Condition, VAR_NAMES};
use super::generate::fields_strategy;
use super::layout::{ControlAdjacency, ControlLayout, ControlOrder};
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
        (any::<Index>(), any::<Index>()),
        ControlOrder::strategy(),
        ControlAdjacency::strategy(),
    )
        .prop_flat_map(
            move |(fields, (control_position, vars_position), order, adjacency)| {
                let (condition, bindings) = verdict_condition(verdict_holds);
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
                        control_order: order,
                        control_adjacency: adjacency,
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
fn verdict_condition(verdict_holds: bool) -> (Condition, Bindings) {
    // A comparison against a constant reaches both verdicts by construction,
    // and `w0` is always bound, so the condition can never short-circuit on an
    // undefined name.
    let constant = 5;
    let (operator, value) = if verdict_holds {
        (CmpOp::Lt, constant - 1)
    } else {
        (CmpOp::Lt, constant + 1)
    };
    (
        Condition::VarCmp(0, operator, constant),
        Bindings::with_count(0, value),
    )
}
