//! JSON-preference extraction from discovered configuration values.

use serde_json::Value;

/// Read an optional JSON preference from one configuration value.
pub(super) fn json_from_value(value: &Value) -> Option<bool> {
    value
        .as_object()
        .and_then(|map| map.get("json"))
        .and_then(Value::as_bool)
}

#[cfg(test)]
mod tests {
    //! Tests for JSON-preference extraction from configuration values.

    use super::*;
    use rstest::rstest;

    /// Every valid and invalid input path of [`json_from_value`].
    ///
    /// The cases cover an object with a boolean, an object carrying a
    /// non-boolean JSON value, an object missing the `json` key, and a
    /// non-object value.
    #[rstest]
    #[case::object_with_bool(serde_json::json!({ "json": true }), Some(true))]
    #[case::object_with_bool_false(serde_json::json!({ "json": false }), Some(false))]
    #[case::object_with_non_bool(serde_json::json!({ "json": "yes" }), None)]
    #[case::object_missing_json(serde_json::json!({ "other": 1 }), None)]
    #[case::non_object(serde_json::json!("plain"), None)]
    fn json_from_value_covers_valid_and_invalid_paths(
        #[case] value: Value,
        #[case] expected: Option<bool>,
    ) {
        assert_eq!(json_from_value(&value), expected);
    }
}
