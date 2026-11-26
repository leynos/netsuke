//! Strategies that construct authored entries and documents.
//!
//! Every generator constructs a legal entry rather than filtering for one, so
//! no draw is discarded and the coverage classification counts real cases.

use super::condition::{Condition, VAR_NAMES};
use super::spec::ControlLayout;
use super::{
    Bindings, BodySpec, DocumentSpec, EntrySpec, ForeachForm, Items, OVERRIDE_NAMES, Section,
};
use crate::json_value_strategy_test_support::json_values;
use crate::manifest::ManifestValue;
use proptest::prelude::*;
use proptest::sample::Index;
use serde_json::Number;

/// Field names a `Target` really carries, used often enough to keep the
/// generated entries recognisable.
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
fn fields_strategy() -> impl Strategy<Value = Vec<(String, ManifestValue)>> {
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
///
/// The condition's own variable is bound to the value the oracle expects, so
/// the condition is live rather than short-circuited. Other names are drawn
/// freely, and a foreach entry may also claim `item` or `index` to exercise
/// override.
fn vars_strategy(
    condition: Option<Condition>,
    is_foreach: bool,
    bindings: Bindings,
) -> impl Strategy<Value = Vec<(String, ManifestValue)>> {
    let mut pool: Vec<&'static str> = VAR_NAMES.to_vec();
    if is_foreach {
        pool.extend(OVERRIDE_NAMES);
    }
    let free_variable = condition.and_then(Condition::variable);
    let extra_pool: Vec<&'static str> = pool
        .into_iter()
        .filter(|name| Some(*name) != free_variable)
        .collect();
    let bound = condition.zip(free_variable).and_then(|(condition, name)| {
        VAR_NAMES
            .iter()
            .position(|candidate| *candidate == name)
            .map(|slot| (condition, slot))
    });
    let bound_value = bound.map(|(condition, slot)| {
        (
            VAR_NAMES[slot].to_owned(),
            Bindings::binding_for(condition, slot, &bindings),
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
                let name = extra_pool[index.index(extra_pool.len())];
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
    if integer_domain {
        proptest::collection::vec(-2i64..10, 0..=MAX_ITEMS)
            .prop_map(Items::Integers)
            .boxed()
    } else {
        proptest::collection::vec(json_values(field_string_strategy()), 0..=MAX_ITEMS)
            .prop_map(Items::Arbitrary)
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
        (any::<Index>(), any::<Index>()),
    )
        .prop_flat_map(
            move |(
                bindings,
                drawn_condition,
                fields,
                form,
                integer_domain,
                has_vars,
                (control_position, vars_position),
            )| {
                let condition = normalise_condition(drawn_condition, has_foreach);
                let condition = has_when.then_some(condition);
                let integer_domain = integer_domain || condition.is_some_and(Condition::reads_item);
                let vars = vars_strategy(condition, has_foreach, bindings.clone());
                // Only a boolean-valued shape has a template form. A template
                // that rendered an integer would read as false, which is not
                // what the condition means.
                let accepts_template = condition.is_some_and(Condition::has_template_form);
                let template = any::<bool>().prop_map(move |drawn| drawn && accepts_template);
                let items: BoxedStrategy<Option<Items>> = if has_foreach {
                    items_strategy(integer_domain).prop_map(Some).boxed()
                } else {
                    Just(None).boxed()
                };
                (vars, template, items).prop_map(move |(vars, template, items)| {
                    let vars = (has_vars || condition.is_some()).then_some(vars);
                    BodySpec {
                        fields: fields.clone(),
                        vars,
                        vars_position,
                        foreach: items.map(|items| (form, items)),
                        when: condition.map(|condition| (condition, template)),
                        control_layout: ControlLayout::drawn(control_position),
                        bindings: bindings.clone(),
                    }
                })
            },
        )
}

/// Generate one body with both control keys decided by the draw.
///
/// A condition is three times as likely as no condition: the condition shapes
/// are what the key-order, filtering, and injection obligations are about, and
/// a coin flip leaves the rarer shapes below their coverage floor.
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
    if has_foreach || !(condition.reads_index() || condition.reads_item()) {
        condition
    } else {
        Condition::Literal(true)
    }
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

/// Generate one static entry, with or without a condition.
pub(in crate::manifest::expand::property_tests) fn static_entry_spec_strategy()
-> impl Strategy<Value = EntrySpec> {
    condition_presence().prop_flat_map(|has_when| entry_with_controls(false, has_when))
}

/// Generate one static entry that carries a condition.
pub(in crate::manifest::expand::property_tests) fn static_condition_entry_spec_strategy()
-> impl Strategy<Value = EntrySpec> {
    entry_with_controls(false, true)
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
