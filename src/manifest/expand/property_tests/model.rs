//! Oracles that predict expansion output from the authored model.
//!
//! Every function here reads the [`EntrySpec`] model and computes what
//! expansion must produce. None of them calls a production helper, and none
//! removes a key from a `serde_json::Map`: the oracle must not share the
//! operation under test.

use super::strategy::{BodySpec, DocumentSpec, EntrySpec, Section};
use crate::manifest::{ManifestMap, ManifestValue};

/// Predict the expanded entries one authored entry produces.
pub(super) fn expected_outputs(spec: &EntrySpec) -> Vec<ManifestValue> {
    spec.body.items().map_or_else(
        || {
            spec.body
                .condition_holds(None, 0)
                .then(|| expected_entry(&spec.body, None))
                .into_iter()
                .collect()
        },
        |items| {
            (0..items.len())
                .filter_map(|index| {
                    let item = items.value_at(index);
                    spec.body
                        .condition_holds(item.as_i64(), index)
                        .then(|| expected_entry(&spec.body, Some(&(item, index))))
                })
                .collect()
        },
    )
}

/// Predict which iterations one authored entry filters out.
///
/// A static entry contributes `None`; a foreach entry contributes the index
/// of each excluded item.
pub(super) fn expected_filtered(body: &BodySpec) -> Vec<Option<usize>> {
    body.items().map_or_else(
        || {
            (!body.condition_holds(None, 0))
                .then_some(None)
                .into_iter()
                .collect()
        },
        |items| {
            (0..items.len())
                .filter(|index| !body.condition_holds(items.value_at(*index).as_i64(), *index))
                .map(Some)
                .collect()
        },
    )
}

/// Predict the whole expanded document.
pub(super) fn expected_document(spec: &DocumentSpec) -> ManifestMap {
    let mut expected = ManifestMap::new();
    for section in [Section::Targets, Section::Actions] {
        let entries: Vec<ManifestValue> = spec
            .entries(section)
            .iter()
            .flat_map(expected_outputs)
            .collect();
        expected.insert(section.name().to_owned(), ManifestValue::Array(entries));
    }
    expected
}

/// Build one expected entry from the model.
fn expected_entry(body: &BodySpec, iteration: Option<&(ManifestValue, usize)>) -> ManifestValue {
    let mut entry = ManifestMap::new();
    for key in body.predicted_keys() {
        if key == "vars" {
            if let Some(vars) = body.predicted_vars(iteration) {
                entry.insert(key, ManifestValue::Object(vars));
            }
        } else if let Some(value) = body.field_value(&key) {
            entry.insert(key, value.clone());
        }
    }
    ManifestValue::Object(entry)
}

/// Compare two JSON values, requiring identical float bit patterns.
///
/// `serde_json::Value` equality treats `-0.0` and `0.0` as equal, and the
/// contract is bit-exact survival through the evaluator, so floats are
/// compared by [`f64::to_bits`] instead.
/// Compare two JSON values, requiring identical numeric representations.
///
/// `serde_json::Value` equality treats `-0.0` and `0.0` as equal, and the
/// contract is bit-exact survival through the evaluator, so floats are
/// compared by [`f64::to_bits`] instead. The two values must also agree on
/// whether they are floats at all: converting a large integer to `f64` rounds
/// it, so comparing `9007199254740993` against `9007199254740992.0` through
/// `as_f64` would report equality for values the evaluator keeps distinct.
pub(super) fn deep_equal(actual: &ManifestValue, expected: &ManifestValue) -> bool {
    match (actual, expected) {
        (ManifestValue::Number(actual_number), ManifestValue::Number(expected_number)) => {
            match (actual_number.is_f64(), expected_number.is_f64()) {
                (true, true) => actual_number
                    .as_f64()
                    .zip(expected_number.as_f64())
                    .is_some_and(|(actual_float, expected_float)| {
                        actual_float.to_bits() == expected_float.to_bits()
                    }),
                (false, false) => actual_number == expected_number,
                // An integer and a float are different representations, and
                // comparing them through `f64` would round the integer.
                (true, false) | (false, true) => false,
            }
        }
        (ManifestValue::Array(actual_items), ManifestValue::Array(expected_items)) => {
            actual_items.len() == expected_items.len()
                && actual_items
                    .iter()
                    .zip(expected_items)
                    .all(|(actual_item, expected_item)| deep_equal(actual_item, expected_item))
        }
        (ManifestValue::Object(actual_map), ManifestValue::Object(expected_map)) => {
            actual_map.len() == expected_map.len()
                && actual_map.iter().all(|(key, value)| {
                    expected_map
                        .get(key)
                        .is_some_and(|other| deep_equal(value, other))
                })
        }
        _ => actual == expected,
    }
}

#[cfg(test)]
mod tests {
    //! Regression cases for the oracle's own comparisons.

    use super::*;
    use serde_json::Number;

    /// Verify an integer never compares equal to a float that rounds to it.
    ///
    /// `9007199254740993` is `2^53 + 1`, the first integer a binary64 cannot
    /// represent. Comparing both through `as_f64` would round the integer onto
    /// the float's value and report equality for two values the evaluator
    /// keeps distinct, so the oracle would accept a real fidelity defect.
    #[test]
    fn integer_and_float_representations_never_compare_equal() {
        let integer = ManifestValue::Number(Number::from(9_007_199_254_740_993_i64));
        let rounded = ManifestValue::Number(
            Number::from_f64(9_007_199_254_740_992.0).expect("the literal is finite"),
        );
        assert!(!deep_equal(&integer, &rounded));
        assert!(!deep_equal(&rounded, &integer));
    }

    /// Verify floats still compare bit for bit, including signed zero.
    #[test]
    fn floats_compare_by_bit_pattern() {
        let positive_zero = ManifestValue::Number(Number::from_f64(0.0).expect("zero is finite"));
        let negative_zero =
            ManifestValue::Number(Number::from_f64(-0.0).expect("negative zero is finite"));
        assert!(!deep_equal(&positive_zero, &negative_zero));
        assert!(deep_equal(&positive_zero, &positive_zero));
    }

    /// Verify integers of the same value still compare equal.
    #[test]
    fn equal_integers_compare_equal() {
        let left = ManifestValue::Number(Number::from(42_i64));
        let right = ManifestValue::Number(Number::from(42_i64));
        assert!(deep_equal(&left, &right));
    }
}
