//! Strategies that construct authored entries and documents.

use super::condition::{Condition, VAR_NAMES, var_name};
use super::layout::{ControlLayout, ControlOrder};
use super::verdict::static_verdict_body;
use super::{
    Bindings, BodySpec, DocumentSpec, EntrySpec, ForeachForm, Items, OVERRIDE_NAMES, Section,
};
use crate::arbitrary_json_test_support::json_values;
use crate::manifest::ManifestValue;
use proptest::prelude::*;
use proptest::sample::Index;
use serde_json::Number;

/// Field names a `Target` really carries, listed once for the generators.
const REAL_FIELD_NAMES: [&str; 11] = [
    "name",
    "command",
    "description",
    "sources",
    "deps",
    "order_only_deps",
    "phony",
    "always",
    "rule",
    "script",
    "conditional",
];

/// Maximum number of authored fields in one entry.
const MAX_FIELDS: usize = 6;

/// Maximum number of authored `vars` entries.
const MAX_VARS: usize = 4;

/// Maximum number of iteration values in one `foreach`.
const MAX_ITEMS: usize = 8;

/// Maximum number of entries in one document section.
const MAX_ENTRIES: usize = 4;

/// Generate one non-control field name.
fn field_key_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => prop::sample::select(REAL_FIELD_NAMES.as_slice()).prop_map(str::to_owned),
        3 => "[a-z][a-z0-9_]{0,7}".prop_map(|key: String| key),
    ]
    .prop_map(|key| {
        if matches!(key.as_str(), "foreach" | "when" | "vars") {
            format!("field_{key}")
        } else {
            key
        }
    })
}

/// Generate the string leaf shared by the JSON strategy.
fn field_string_strategy() -> BoxedStrategy<String> {
    prop_oneof![
        "[a-z][a-z0-9_]{0,7}".prop_map(|key: String| key),
        Just("{{ item }}".to_owned()),
        Just("{{ index }}".to_owned()),
        Just("plain text".to_owned()),
    ]
    .boxed()
}

/// Generate one non-control field value.
fn field_value_strategy() -> impl Strategy<Value = ManifestValue> {
    prop_oneof![
        6 => json_values(field_string_strategy()),
        1 => Just(ManifestValue::from(i64::MIN)),
        1 => Just(ManifestValue::from(i64::MAX)),
        1 => Just(ManifestValue::Number(Number::from(u64::MAX))),
    ]
}

/// Generate up to [`MAX_FIELDS`] fields, keeping the first of each name.
pub(super) fn fields_strategy() -> impl Strategy<Value = Vec<(String, ManifestValue)>> {
    proptest::collection::vec(
        (field_key_strategy(), field_value_strategy()),
        0..=MAX_FIELDS,
    )
    .prop_map(|drawn| {
        let mut seen: Vec<String> = Vec::new();
        drawn
            .into_iter()
            .filter(|(key, _)| {
                if seen.contains(key) {
                    false
                } else {
                    seen.push(key.clone());
                    true
                }
            })
            .collect()
    })
}

/// Generate the authored `vars` entries for one entry.
pub(super) fn vars_strategy(
    condition: Option<Condition>,
    is_foreach: bool,
    bindings: &Bindings,
) -> impl Strategy<Value = Vec<(String, ManifestValue)>> + use<> {
    let mut pool: Vec<&'static str> = VAR_NAMES.to_vec();
    if is_foreach {
        pool.extend(OVERRIDE_NAMES);
    }
    let free_variable = condition.and_then(Condition::variable);
    let extra_pool: Vec<&'static str> = pool
        .into_iter()
        .filter(|name| Some(*name) != free_variable)
        .collect();
    let bound = condition.zip(free_variable).and_then(|(shape, name)| {
        VAR_NAMES
            .iter()
            .position(|candidate| *candidate == name)
            .map(|slot| (shape, slot))
    });
    let bound_value = bound.map(|(shape, slot)| {
        (
            var_name(slot).to_owned(),
            Bindings::binding_for(shape, slot, bindings),
        )
    });
    (
        proptest::collection::vec(any::<Index>(), 0..=MAX_VARS),
        proptest::collection::vec(field_value_strategy(), 0..=MAX_VARS),
    )
        .prop_map(move |(indexes, values)| {
            let mut chosen: Vec<(String, ManifestValue)> = bound_value.iter().cloned().collect();
            if extra_pool.is_empty() {
                return chosen;
            }
            for (position, index) in indexes.iter().enumerate() {
                let name = extra_pool
                    .get(index.index(extra_pool.len()))
                    .copied()
                    .unwrap_or("w0");
                if chosen.iter().any(|(key, _)| key == name) {
                    continue;
                }
                let value = values.get(position).cloned().unwrap_or(ManifestValue::Null);
                chosen.push((name.to_owned(), value));
            }
            chosen
        })
}

/// Generate the iteration values for one entry.
fn items_strategy(integer_domain: bool) -> BoxedStrategy<Items> {
    let non_empty = 1..=MAX_ITEMS;
    if integer_domain {
        prop_oneof![
            1 => Just(Items::Integers(Vec::new())),
            2 => proptest::collection::vec(-2i64..10, non_empty).prop_map(Items::Integers),
        ]
        .boxed()
    } else {
        // A null item is the only way to reach that kind through the string
        // form, and a uniform draw rarely makes one, so it gets its own arm.
        prop_oneof![
            1 => Just(Items::Arbitrary(Vec::new())),
            2 => proptest::collection::vec(json_values(field_string_strategy()), non_empty)
                .prop_map(Items::Arbitrary),
            2 => proptest::collection::vec(json_values(field_string_strategy()), 0..=3)
                .prop_map(|mut values| {
                    values.push(ManifestValue::Null);
                    Items::Arbitrary(values)
                }),
        ]
        .boxed()
    }
}

/// Generate one authored entry body with the caller fixing the control keys.
fn body_strategy(has_foreach: bool, has_when: bool) -> impl Strategy<Value = BodySpec> {
    (
        Bindings::strategy(),
        Condition::strategy(),
        fields_strategy(),
        ForeachForm::strategy(),
        any::<bool>(),
        any::<bool>(),
        (any::<Index>(), any::<Index>(), any::<Index>()),
        ControlOrder::strategy(),
    )
        .prop_flat_map(
            move |(
                bindings,
                drawn_condition,
                fields,
                form,
                integer_domain,
                has_vars,
                (control_position, second_control_position, vars_position),
                control_order,
            )| {
                let normalised = normalise_condition(drawn_condition, has_foreach);
                let maybe_condition = has_when.then_some(normalised);
                let integer_items =
                    integer_domain || maybe_condition.is_some_and(Condition::reads_item);
                let vars = vars_strategy(maybe_condition, has_foreach, &bindings);
                // Only a boolean-valued shape has a template form; a rendered
                // integer would read as false.
                let accepts_template = maybe_condition.is_some_and(Condition::has_template_form);
                let template = any::<bool>().prop_map(move |drawn| drawn && accepts_template);
                let item_values: BoxedStrategy<Option<Items>> = if has_foreach {
                    items_strategy(integer_items).prop_map(Some).boxed()
                } else {
                    Just(None).boxed()
                };
                (vars, template, item_values).prop_map(
                    move |(drawn_vars, drawn_template, drawn_items)| {
                        let final_vars =
                            (has_vars || maybe_condition.is_some()).then_some(drawn_vars);
                        BodySpec {
                            fields: fields.clone(),
                            vars: final_vars,
                            vars_position,
                            foreach: drawn_items.map(|values| (form, values)),
                            when: maybe_condition.map(|shape| (shape, drawn_template)),
                            control_layout: ControlLayout::drawn(control_position),
                            second_control_layout: ControlLayout::drawn(second_control_position),
                            control_order,
                            bindings: bindings.clone(),
                        }
                    },
                )
            },
        )
}

/// Generate one body with both control keys decided by the draw.
fn body_choices() -> impl Strategy<Value = BodySpec> {
    (any::<bool>(), condition_presence())
        .prop_flat_map(|(has_foreach, has_when)| body_strategy(has_foreach, has_when))
}

/// Generate whether a condition is present, biased towards presence.
fn condition_presence() -> impl Strategy<Value = bool> {
    prop_oneof![3 => Just(true), 1 => Just(false)]
}

/// Generate one entry with the caller fixing the control keys.
fn entry_with_controls(has_foreach: bool, has_when: bool) -> impl Strategy<Value = EntrySpec> {
    (Section::strategy(), body_strategy(has_foreach, has_when))
        .prop_map(|(section, body)| EntrySpec { section, body })
}

/// Replace a condition that a static entry cannot legally carry.
fn normalise_condition(condition: Condition, has_foreach: bool) -> Condition {
    if has_foreach {
        return condition;
    }
    if condition.reads_iteration() {
        return Condition::Literal(true);
    }
    condition
}

/// Generate one entry with its section.
pub(in crate::manifest::expand::property_tests) fn entry_spec_strategy()
-> impl Strategy<Value = EntrySpec> {
    (Section::strategy(), body_choices()).prop_map(|(section, body)| EntrySpec { section, body })
}

/// Generate one foreach entry, with or without a condition.
pub(in crate::manifest::expand::property_tests) fn foreach_entry_spec_strategy()
-> impl Strategy<Value = EntrySpec> {
    condition_presence().prop_flat_map(|has_when| entry_with_controls(true, has_when))
}

/// Generate one static entry that always carries a condition.
pub(in crate::manifest::expand::property_tests) fn static_condition_entry_spec_strategy()
-> impl Strategy<Value = EntrySpec> {
    (Section::strategy(), any::<bool>()).prop_flat_map(|(section, verdict_holds)| {
        static_verdict_body(verdict_holds).prop_map(move |body| EntrySpec { section, body })
    })
}

/// Generate a document whose entries are all static.
pub(in crate::manifest::expand::property_tests) fn static_document_spec_strategy()
-> impl Strategy<Value = DocumentSpec> {
    let entries = || {
        proptest::collection::vec(
            (any::<bool>()).prop_flat_map(|has_when| body_strategy(false, has_when)),
            0..=MAX_ENTRIES,
        )
    };
    (entries(), entries()).prop_map(|(targets, actions)| DocumentSpec {
        targets: targets
            .into_iter()
            .map(|body| EntrySpec {
                section: Section::Targets,
                body,
            })
            .collect(),
        actions: actions
            .into_iter()
            .map(|body| EntrySpec {
                section: Section::Actions,
                body,
            })
            .collect(),
    })
}

/// Generate a whole document, forcing each list's section.
pub(in crate::manifest::expand::property_tests) fn document_spec_strategy()
-> impl Strategy<Value = DocumentSpec> {
    (
        proptest::collection::vec(body_choices(), 0..=MAX_ENTRIES),
        proptest::collection::vec(body_choices(), 0..=MAX_ENTRIES),
    )
        .prop_map(|(targets, actions)| DocumentSpec {
            targets: targets
                .into_iter()
                .map(|body| EntrySpec {
                    section: Section::Targets,
                    body,
                })
                .collect(),
            actions: actions
                .into_iter()
                .map(|body| EntrySpec {
                    section: Section::Actions,
                    body,
                })
                .collect(),
        })
}
