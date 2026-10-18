//! The generated domain for `OBL-ACTION`.
//!
//! The obligation is stated over pairs of `Action` values, and the domain is
//! what decides whether the statement is worth testing. Three arms are drawn:
//! a value against its clone, two independent values, and a base value with
//! exactly one field mutated. The third arm is the only one that can detect a
//! hasher ignoring a field, so its mutation is total and guaranteed to differ
//! rather than rejection-sampled until it does.
//!
//! This file is a child rather than part of the parent because the parent
//! would otherwise exceed the 400-line `module_max_lines` ceiling. The seam is
//! the one the repository already uses for the same reason in
//! `ninja_gen::determinism::property_tests::defaults::generator`: generation
//! here, properties and their evidence there.

use crate::ast::{Recipe, StringOrList};
use crate::ir::Action;
use proptest::prelude::*;

/// One generated pair of actions together with the arm that formed it.
///
/// `Debug` is required rather than incidental: Proptest renders a failing case
/// by formatting it, so a case type that cannot be printed turns every failure
/// into a compile error at the macro instead of a shrinking counter-example.
#[derive(Clone, Debug)]
pub(super) struct ActionCase {
    /// The first value of the pair.
    left: Action,
    /// The second value of the pair.
    right: Action,
    /// How `right` was obtained from `left`.
    pub(super) arm: Arm,
}

impl ActionCase {
    /// Return the pair this case compares.
    pub(super) fn pair(&self) -> (Action, Action) {
        (self.left.clone(), self.right.clone())
    }
}

/// How the second value of a case relates to the first.
///
/// The arm is part of the case rather than a separate strategy so that the
/// coverage tally can observe which arms fired, and so that a reported failure
/// names the arm that produced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Arm {
    /// The pair is a value and its clone.
    Clone,
    /// The pair is two independent draws.
    Independent,
    /// The pair differs in exactly the field this arm names.
    Mutated(Field),
}

/// Every arm, with each of the eight choices equally likely.
///
/// Eight rather than three because the mutated arm carries six fields, and
/// drawing them uniformly keeps every field reachable in a 512-case run.
const ARMS: &[Arm] = &[
    Arm::Clone,
    Arm::Independent,
    Arm::Mutated(Field::Recipe),
    Arm::Mutated(Field::Description),
    Arm::Mutated(Field::Depfile),
    Arm::Mutated(Field::DepsFormat),
    Arm::Mutated(Field::Pool),
    Arm::Mutated(Field::Restat),
];

/// One of the six fields an `Action` carries, in declaration order.
///
/// The variants exist so that the single-field arm can name exactly which
/// field it perturbed, and so that the tally can prove each was reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Field {
    /// The recipe itself.
    Recipe,
    /// The optional Ninja `description`.
    Description,
    /// The optional Ninja `depfile`.
    Depfile,
    /// The optional Ninja `deps` format.
    DepsFormat,
    /// The optional Ninja `pool`.
    Pool,
    /// The Ninja `restat` flag.
    Restat,
}

impl Field {
    /// Every field, in the order the struct declares them.
    pub(super) const ALL: [Self; 6] = [
        Self::Recipe,
        Self::Description,
        Self::Depfile,
        Self::DepsFormat,
        Self::Pool,
        Self::Restat,
    ];

    /// Return `action` with this field changed and every other field intact.
    ///
    /// Each replacement is guaranteed to differ from the original rather than
    /// drawn until it does. A mutation that happened to be a no-op would make
    /// the arm's evidence vacuous without any assertion noticing, and rejection
    /// sampling inside a strategy is the defect this repository's proptest
    /// guidance names.
    fn mutate(self, action: &Action) -> Action {
        let mut mutated = action.clone();
        match self {
            Self::Recipe => mutated.recipe = other_recipe(&action.recipe),
            Self::Description => {
                mutated.description = other_optional(action.description.as_deref());
            }
            Self::Depfile => mutated.depfile = other_optional(action.depfile.as_deref()),
            Self::DepsFormat => mutated.deps_format = other_optional(action.deps_format.as_deref()),
            Self::Pool => mutated.pool = other_optional(action.pool.as_deref()),
            Self::Restat => mutated.restat = !action.restat,
        }
        mutated
    }
}

/// Return a recipe of a different variant from `recipe`.
///
/// Cycling the variant is the largest structural change available, so a digest
/// would have to ignore the whole recipe — tag included — for the mutation to
/// be invisible. Constructing a second value of the same variant would need a
/// guaranteed-different payload, which some payloads do not offer.
fn other_recipe(recipe: &Recipe) -> Recipe {
    match recipe {
        Recipe::Command { .. } => Recipe::Script {
            script: "echo mutated".to_owned(),
        },
        Recipe::Script { .. } => Recipe::Rule {
            rule: StringOrList::String("mutated".to_owned()),
        },
        Recipe::Rule { .. } => Recipe::Command {
            command: StringOrList::String("echo mutated".to_owned()),
        },
    }
}

/// Return a value for an optional string field that differs from `current`.
///
/// `None` is itself a legitimate difference — the field is optional — so an
/// action carrying `Some` is mutated by clearing it and one carrying `None` by
/// setting it. That is total over the field's domain without ever producing
/// the original.
fn other_optional(current: Option<&str>) -> Option<String> {
    match current {
        Some(_) => None,
        None => Some("mutated".to_owned()),
    }
}

/// A strategy for one case, with each arm reachable at a known rate.
///
/// The arm decides how the second value is obtained, so the clone arm is
/// guaranteed equal and the mutated arm is guaranteed to differ in one field.
/// Independently drawing the arm and then hoping the values happened to line
/// up would leave both guarantees to chance.
pub(super) fn action_case() -> impl Strategy<Value = ActionCase> {
    (prop::sample::select(ARMS), action_strategy()).prop_flat_map(|(arm, left)| {
        let right = match arm {
            Arm::Clone => Just(left.clone()).boxed(),
            Arm::Independent => action_strategy().boxed(),
            Arm::Mutated(field) => Just(field.mutate(&left)).boxed(),
        };
        (Just(left), right, Just(arm)).prop_map(|(first, second, label)| ActionCase {
            left: first,
            right: second,
            arm: label,
        })
    })
}

/// A complete `Action` drawn field by field.
///
/// All six fields are generated, including the four the lowering pipeline
/// cannot currently set. That is the point of this suite: the hasher is a
/// general function over `Action`, and a property that only exercised what
/// today's manifests can build would leave the other four fields unguarded.
pub(super) fn action_strategy() -> impl Strategy<Value = Action> {
    (
        recipe_strategy(),
        prop::option::of("[a-z ]{0,16}"),
        prop::option::of("[a-z/]{0,16}"),
        prop::option::of("[a-z]{0,8}"),
        prop::option::of("[a-z]{0,8}"),
        any::<bool>(),
    )
        .prop_map(
            |(recipe, description, depfile, deps_format, pool, restat)| Action {
                recipe,
                description,
                depfile,
                deps_format,
                pool,
                restat,
            },
        )
}

/// One executable recipe.
///
/// Every variant is drawn, including `Rule`, because the hasher serialises the
/// recipe as an enum tag plus payload and a wrong tag would be invisible if
/// only one variant were generated. The two `Command` arms draw a scalar and a
/// list separately: `StringOrList` serialises its *shape* as well as its
/// contents, so a scalar and a one-element list must not be conflated by the
/// generator even though they render alike.
fn recipe_strategy() -> impl Strategy<Value = Recipe> {
    prop_oneof![
        "[a-z ]{0,16}".prop_map(|command| Recipe::Command {
            command: StringOrList::String(command),
        }),
        prop::collection::vec("[a-z ]{0,16}", 0..=3).prop_map(|items| Recipe::Command {
            command: StringOrList::List(items),
        }),
        "[a-z ]{0,16}".prop_map(|script| Recipe::Script { script }),
        "[a-z-]{0,16}".prop_map(|rule| Recipe::Rule {
            rule: StringOrList::String(rule),
        }),
    ]
}
