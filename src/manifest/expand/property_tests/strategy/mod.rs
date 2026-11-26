//! Generators for authored manifest entries and the model an oracle reads.
//!
//! A property about expansion is only as good as the shape it is handed, so
//! the generator produces an [`EntrySpec`]: an explicit model of what the
//! author wrote, from which [`EntrySpec::render`] builds the `ManifestValue`
//! the production code consumes. Oracles then read the model rather than the
//! rendered map, which keeps them structurally independent of the code under
//! test and lets them predict key order without inspecting an actual key.
//!
//! The generator constructs valid entries; it never filters. Positions are
//! drawn with `proptest::sample::Index` so they shrink well and can be applied
//! after duplicate field names are removed.
//!
//! Two invariants keep a generated entry legal without discarding draws:
//!
//! - a static entry carries neither `index` nor `item` references, because the
//!   strict-undefined environment would reject them rather than exclude the
//!   entry;
//! - a condition's free variable is always bound, because `MiniJinja` short
//!   circuits and an unbound name would hide the defect it is meant to expose.

pub(in crate::manifest::expand::property_tests) mod condition;
mod generate;
mod layout;
mod spec;
mod verdict;

pub(in crate::manifest::expand::property_tests) use condition::Condition;
use condition::VAR_NAMES;
pub(in crate::manifest::expand::property_tests) use generate::{
    document_spec_strategy, entry_spec_strategy, foreach_entry_spec_strategy,
    static_condition_entry_spec_strategy, static_entry_spec_strategy,
};
pub(in crate::manifest::expand::property_tests) use layout::ControlOrder;
pub(in crate::manifest::expand::property_tests) use spec::{BodySpec, DocumentSpec, EntrySpec};

use crate::manifest::ManifestValue;
use proptest::prelude::*;
use serde_json::Number;

/// User `vars` names a foreach entry may carry to exercise override.
const OVERRIDE_NAMES: [&str; 2] = ["item", "index"];

/// Which manifest section a generated entry belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::manifest::expand::property_tests) enum Section {
    /// The `targets` section.
    Targets,
    /// The `actions` section.
    Actions,
}

impl Section {
    /// Generate one section.
    pub(super) fn strategy() -> impl Strategy<Value = Self> {
        prop_oneof![Just(Self::Targets), Just(Self::Actions)]
    }

    /// Name the section as the manifest spells it.
    pub(in crate::manifest::expand::property_tests) const fn name(self) -> &'static str {
        match self {
            Self::Targets => "targets",
            Self::Actions => "actions",
        }
    }
}

/// Which spelling of `foreach` an entry uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::manifest::expand::property_tests) enum ForeachForm {
    /// A YAML sequence, which the loader sees as a JSON array.
    InlineArray,
    /// A string holding a Jinja list literal.
    ListLiteral,
}

impl ForeachForm {
    /// Generate one spelling.
    pub(super) fn strategy() -> impl Strategy<Value = Self> {
        prop_oneof![Just(Self::InlineArray), Just(Self::ListLiteral)]
    }
}

/// The iteration values a generated `foreach` supplies.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::manifest::expand::property_tests) enum Items {
    /// Integers in `-2..10`, so conditions may compare against `item`.
    Integers(Vec<i64>),
    /// Arbitrary JSON values, so conditions may read `index` only.
    Arbitrary(Vec<ManifestValue>),
}

impl Items {
    /// Count the iteration values.
    pub(in crate::manifest::expand::property_tests) fn len(&self) -> usize {
        match self {
            Self::Integers(values) => values.len(),
            Self::Arbitrary(values) => values.len(),
        }
    }

    /// Render the value at `index` as the evaluator would return it.
    pub(in crate::manifest::expand::property_tests) fn value_at(
        &self,
        index: usize,
    ) -> ManifestValue {
        match self {
            Self::Integers(values) => values.get(index).map_or(ManifestValue::Null, |value| {
                ManifestValue::Number(Number::from(*value))
            }),
            Self::Arbitrary(values) => values.get(index).cloned().unwrap_or(ManifestValue::Null),
        }
    }

    /// Render the whole sequence as an inline JSON array.
    pub(super) fn inline_array(&self) -> ManifestValue {
        match self {
            Self::Integers(values) => ManifestValue::Array(
                values
                    .iter()
                    .map(|value| ManifestValue::Number(Number::from(*value)))
                    .collect(),
            ),
            Self::Arbitrary(values) => ManifestValue::Array(values.clone()),
        }
    }

    /// Render the whole sequence as a Jinja list literal.
    pub(super) fn list_literal(&self) -> String {
        let rendered: Vec<String> = match self {
            Self::Integers(values) => values.iter().map(i64::to_string).collect(),
            Self::Arbitrary(values) => values.iter().map(jinja_literal).collect(),
        };
        format!("[{}]", rendered.join(", "))
    }
}

/// Render one JSON value as Jinja source for the list-literal form.
fn jinja_literal(value: &ManifestValue) -> String {
    match value {
        ManifestValue::Null => "none".to_owned(),
        ManifestValue::Bool(flag) => flag.to_string(),
        ManifestValue::Number(number) => number.to_string(),
        ManifestValue::String(text) => quoted(text),
        ManifestValue::Array(items) => {
            let rendered: Vec<String> = items.iter().map(jinja_literal).collect();
            format!("[{}]", rendered.join(", "))
        }
        ManifestValue::Object(map) => {
            let rendered: Vec<String> = map
                .iter()
                .map(|(key, item)| format!("{}: {}", quoted(key), jinja_literal(item)))
                .collect();
            format!("{{{}}}", rendered.join(", "))
        }
    }
}

/// Quote one string as a Jinja string literal.
///
/// The generated strings are ASCII and carry no quotes, but escaping the two
/// characters that would end or break a literal costs nothing and keeps the
/// printer honest if the leaf pool grows.
fn quoted(text: &str) -> String {
    let mut rendered = String::with_capacity(text.len() + 2);
    rendered.push('"');
    for character in text.chars() {
        match character {
            '"' => rendered.push_str("\\\""),
            '\\' => rendered.push_str("\\\\"),
            other => rendered.push(other),
        }
    }
    rendered.push('"');
    rendered
}

/// The values a generated condition sees for its free variables.
#[derive(Clone, Debug)]
pub(in crate::manifest::expand::property_tests) struct Bindings {
    /// Boolean values for `w0`–`w3`.
    flags: [bool; 4],
    /// Integer values for `w0`–`w3`.
    counts: [i64; 4],
}

impl Bindings {
    /// Generate one binding set.
    pub(super) fn strategy() -> impl Strategy<Value = Self> {
        (
            proptest::array::uniform4(any::<bool>()),
            proptest::array::uniform4(-2i64..10),
        )
            .prop_map(|(flags, counts)| Self { flags, counts })
    }

    /// Build cleared bindings holding one integer value.
    pub(super) fn with_count(slot: usize, value: i64) -> Self {
        let mut bindings = Self {
            flags: [false; VAR_NAMES.len()],
            counts: [0i64; VAR_NAMES.len()],
        };
        if let Some(entry) = bindings.counts.get_mut(slot) {
            *entry = value;
        }
        bindings
    }

    /// Build cleared bindings holding one boolean value.
    pub(super) fn with_flag(slot: usize, value: bool) -> Self {
        let mut bindings = Self::with_count(slot, 0);
        if let Some(entry) = bindings.flags.get_mut(slot) {
            *entry = value;
        }
        bindings
    }

    /// Read the boolean value one variable name holds.
    ///
    /// Slots are drawn from `0..4`; the default keeps an out-of-range slot
    /// from panicking, so a generator defect shows up as a property failure
    /// rather than as a crashed test.
    pub(in crate::manifest::expand::property_tests) fn flag(&self, slot: usize) -> bool {
        self.flags.get(slot).copied().unwrap_or_default()
    }

    /// Read the integer value one variable name holds.
    pub(in crate::manifest::expand::property_tests) fn count(&self, slot: usize) -> i64 {
        self.counts.get(slot).copied().unwrap_or_default()
    }

    /// Render the value a condition reading one of its names must see.
    pub(super) fn binding_for(condition: Condition, slot: usize, bindings: &Self) -> ManifestValue {
        match condition {
            Condition::Flag(_) | Condition::NotFlag(_) => ManifestValue::Bool(bindings.flag(slot)),
            _ => ManifestValue::Number(Number::from(bindings.count(slot))),
        }
    }
}

#[cfg(test)]
mod tests;
