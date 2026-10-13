//! Property tests for occurrence-level project authority.
//!
//! These properties vary layer count and order while giving every occurrence
//! the same physical path, proving authority follows its discovery provenance.

use super::*;
use proptest::prelude::*;
use serde_json::{Map, Value, json};

proptest! {
    /// Quarantine grants per occurrence across mixed roots and arbitrary order.
    #[test]
    fn environment_grants_follow_occurrence_provenance(
        extra_cases in prop::collection::vec((0u8..5, 0u8..3, 0u8..4), 0..20),
        rotation_seed in 0usize..32,
        reverse in any::<bool>(),
    ) {
        // Include every authority category and both grant validation outcomes
        // even when the generated suffix shrinks to an empty vector.
        let mut cases = vec![(0, 1, 2), (1, 2, 1), (2, 1, 1), (3, 2, 1), (4, 1, 1)];
        cases.extend(extra_cases);
        let rotation_index = rotation_seed.min(cases.len() - 1);
        cases.rotate_left(rotation_index);
        if reverse {
            cases.reverse();
        }

        let mut occurrences = Vec::with_capacity(cases.len());
        for (role, grant_kind, grant_count) in &cases {
            let (scope, project_chain) = match *role {
                0 | 4 => (FileScope::Operator, false),
                1 => (FileScope::Operator, true),
                2 => (FileScope::Project, true),
                _ => (FileScope::Project, false),
            };
            let mut fields = Map::from_iter([
                ("env_block_var".to_owned(), json!(["BLOCKED_TOKEN"])),
                ("fetch_allow_host".to_owned(), json!(["shared.example"])),
                ("trust_project_fetch_policy".to_owned(), json!(true)),
            ]);
            match *grant_kind {
                0 => {}
                1 => {
                    fields.insert(
                        "env_allow_var".to_owned(),
                        json!(vec!["CI_TOKEN"; usize::from(*grant_count)]),
                    );
                }
                _ => {
                    fields.insert("env_allow_var".to_owned(), json!("malformed"));
                }
            }
            occurrences.push(ScopedFileLayer {
                layer: MergeLayer::file(
                    Cow::Owned(Value::Object(fields)),
                    Some("shared.toml".into()),
                ),
                scope,
                project_chain,
            });
        }

        let resolved = retain_layers_and_resolve_json(occurrences);
        let expected_errors = cases
            .iter()
            .filter(|(role, grant_kind, _)| matches!(*role, 1..=3) && *grant_kind == 2)
            .count();
        prop_assert_eq!(resolved.errors.len(), expected_errors);
        prop_assert!(
            resolved.errors.iter().all(|error| matches!(
                error.as_ref(),
                OrthoError::Validation { key, .. } if key == "env_allow_var"
            )),
            "only malformed project grants should produce validation errors"
        );

        let mut values = Vec::new();
        for layer in resolved.layers {
            prop_assert_eq!(
                layer.path().map(camino::Utf8Path::as_str),
                Some("shared.toml")
            );
            values.push(layer.into_value());
        }

        prop_assert_eq!(values.len(), cases.len());
        for ((role, grant_kind, grant_count), value) in cases.iter().zip(values) {
            let is_quarantined = matches!(*role, 1..=3);
            let mut expected_fields = Map::from_iter([
                ("env_block_var".to_owned(), json!(["BLOCKED_TOKEN"])),
                ("fetch_allow_host".to_owned(), json!(["shared.example"])),
                ("trust_project_fetch_policy".to_owned(), json!(true)),
            ]);
            match (*grant_kind, is_quarantined) {
                (0, _) | (1, true) => {}
                (1, false) => {
                    expected_fields.insert(
                        "env_allow_var".to_owned(),
                        json!(vec!["CI_TOKEN"; usize::from(*grant_count)]),
                    );
                }
                _ => {
                    expected_fields.insert("env_allow_var".to_owned(), json!("malformed"));
                }
            }
            if matches!(*role, 2..=3) {
                expected_fields.remove("fetch_allow_host");
                expected_fields.remove("trust_project_fetch_policy");
            }
            prop_assert_eq!(
                value,
                Value::Object(expected_fields),
                "provenance reference model mismatch for {}, {}",
                role,
                grant_kind
            );
        }
    }
}
