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

    #[test]
    fn reads_json_bool() {
        assert_eq!(
            json_from_value(&serde_json::json!({ "json": true })),
            Some(true)
        );
        assert_eq!(
            json_from_value(&serde_json::json!({ "json": false })),
            Some(false)
        );
    }

    #[test]
    fn ignores_non_bool_json() {
        assert_eq!(json_from_value(&serde_json::json!({ "json": "yes" })), None);
    }
}
