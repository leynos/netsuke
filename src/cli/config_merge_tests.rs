//! Unit tests for private helpers in `config_merge`.

use super::*;
use rstest::rstest;
use serde_json::json;

// ---------------------------------------------------------------------------
// is_empty_value
// ---------------------------------------------------------------------------

#[test]
fn is_empty_value_accepts_empty_object() {
    assert!(is_empty_value(&json!({})));
}

#[rstest]
#[case::string(json!("hello"))]
#[case::number(json!(42))]
#[case::null(json!(null))]
#[case::boolean(json!(true))]
#[case::array(json!([]))]
fn is_empty_value_rejects_non_object_types(#[case] value: serde_json::Value) {
    assert!(!is_empty_value(&value));
}

#[test]
fn is_empty_value_rejects_populated_object() {
    assert!(!is_empty_value(&json!({"theme": "ascii"})));
}

// ---------------------------------------------------------------------------
// diag_json_from_layer
// ---------------------------------------------------------------------------

#[test]
fn diag_json_from_layer_returns_none_for_non_object() {
    assert_eq!(diag_json_from_layer(&json!("hello")), None);
}

#[test]
fn diag_json_from_layer_returns_none_when_neither_field_present() {
    assert_eq!(diag_json_from_layer(&json!({"theme": "ascii"})), None);
}

#[rstest]
#[case::true_value(json!({"diag_json": true}), Some(true))]
#[case::false_value(json!({"diag_json": false}), Some(false))]
fn diag_json_from_layer_reads_diag_json_bool(
    #[case] layer: serde_json::Value,
    #[case] expected: Option<bool>,
) {
    assert_eq!(diag_json_from_layer(&layer), expected);
}

#[test]
fn diag_json_from_layer_returns_none_for_non_bool_diag_json() {
    assert_eq!(diag_json_from_layer(&json!({"diag_json": "yes"})), None);
}

#[rstest]
#[case::json_format(json!({"output_format": "json"}), Some(true))]
#[case::human_format(json!({"output_format": "human"}), Some(false))]
fn diag_json_from_layer_prefers_output_format_over_diag_json(
    #[case] layer: serde_json::Value,
    #[case] expected: Option<bool>,
) {
    assert_eq!(diag_json_from_layer(&layer), expected);
}

#[test]
fn diag_json_from_layer_output_format_wins_over_diag_json() {
    let layer = json!({"output_format": "human", "diag_json": true});
    assert_eq!(
        diag_json_from_layer(&layer),
        Some(false),
        "output_format should take precedence over diag_json"
    );
}

#[test]
fn diag_json_from_layer_ignores_invalid_output_format() {
    let layer = json!({"output_format": "tap", "diag_json": true});
    assert_eq!(
        diag_json_from_layer(&layer),
        Some(true),
        "invalid output_format should fall through to diag_json"
    );
}
