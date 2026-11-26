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
    match spec.body.items() {
        Some(items) => (0..items.len())
            .filter_map(|index| {
                let item = items.value_at(index);
                spec.body
                    .condition_holds(item.as_i64(), index)
                    .then(|| expected_entry(&spec.body, Some((item, index))))
            })
            .collect(),
        None => spec
            .body
            .condition_holds(None, 0)
            .then(|| expected_entry(&spec.body, None))
            .into_iter()
            .collect(),
    }
}

/// Predict which iterations one authored entry filters out.
///
/// A static entry contributes `None`; a foreach entry contributes the index
/// of each excluded item.
pub(super) fn expected_filtered(body: &BodySpec) -> Vec<Option<usize>> {
    match body.items() {
        Some(items) => (0..items.len())
            .filter(|index| !body.condition_holds(items.value_at(*index).as_i64(), *index))
            .map(Some)
            .collect(),
        None => (!body.condition_holds(None, 0))
            .then_some(None)
            .into_iter()
            .collect(),
    }
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
fn expected_entry(body: &BodySpec, iteration: Option<(ManifestValue, usize)>) -> ManifestValue {
    let mut entry = ManifestMap::new();
    for key in body.predicted_keys() {
        if key == "vars" {
            if let Some(vars) = body.predicted_vars(iteration.clone()) {
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
pub(super) fn deep_equal(left: &ManifestValue, right: &ManifestValue) -> bool {
    match (left, right) {
        (ManifestValue::Number(left), ManifestValue::Number(right)) => {
            if left.is_f64() || right.is_f64() {
                left.as_f64()
                    .zip(right.as_f64())
                    .is_some_and(|(left, right)| left.to_bits() == right.to_bits())
            } else {
                left == right
            }
        }
        (ManifestValue::Array(left), ManifestValue::Array(right)) => {
            left.len() == right.len() && left.iter().zip(right).all(|(l, r)| deep_equal(l, r))
        }
        (ManifestValue::Object(left), ManifestValue::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, value)| {
                    right.get(key).is_some_and(|other| deep_equal(value, other))
                })
        }
        _ => left == right,
    }
}
