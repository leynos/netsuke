//! Property tests for bounded CLI override-path diagnostics.
//!
//! The logging observer must identify arbitrary nested override keys without
//! serializing or retaining the caller-controlled leaf values.

use super::merge_observability::collect_override_leaf_paths;
use proptest::prelude::*;
use serde_json::{Map, Value};

/// Build one generated nested override whose leaf value must remain unobserved.
fn nested_override(root_key: String, leaf_key: String, value: String) -> Value {
    let mut nested = Map::new();
    nested.insert(leaf_key, Value::String(value));
    let mut root = Map::new();
    root.insert(root_key, Value::Object(nested));
    Value::Object(root)
}

proptest! {
    /// Arbitrary nested keys become paths while their arbitrary values stay absent.
    #[test]
    fn override_leaf_paths_do_not_echo_values(
        root_key in "[a-z]{1,8}",
        leaf_key in "[a-z]{1,8}",
        value in ".{0,64}",
    ) {
        let raw_value = format!("secret:{value}");
        let paths = collect_override_leaf_paths(&nested_override(
            root_key.clone(),
            leaf_key.clone(),
            raw_value.clone(),
        ));

        prop_assert!(paths.iter().all(|path| !path.contains(&raw_value)));
        prop_assert_eq!(paths, vec![format!("{root_key}.{leaf_key}")]);
    }
}
