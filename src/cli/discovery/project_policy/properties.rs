//! Property tests for occurrence-level project authority.
//!
//! These properties vary layer count and order while giving every occurrence
//! the same physical path, proving authority follows its discovery provenance.

use super::*;
use crate::host_pattern::HostPattern;
use proptest::prelude::*;
use serde_json::{Map, Value, json};

proptest! {
    /// Fold project fetch grants in dependency order and quarantine every layer.
    #[test]
    fn project_fetch_requests_follow_dependency_first_fold(
        requests in prop::collection::vec(
            (
                prop::option::of(any::<bool>()),
                prop::option::of(prop::collection::vec(
                    prop_oneof![
                        Just("http".to_owned()),
                        Just("https".to_owned()),
                        Just("ftp".to_owned()),
                    ],
                    0..4,
                )),
                prop::option::of(prop::collection::vec(0u16..1000, 0..4)),
            ),
            1..9,
        ),
    ) {
        let expected_default_deny = requests
            .iter()
            .filter_map(|(default_deny, _, _)| *default_deny)
            .next_back();
        let expected_schemes = requests
            .iter()
            .flat_map(|(_, schemes, _)| schemes.iter().flatten().cloned())
            .collect::<Vec<_>>();
        let expected_hosts = requests
            .iter()
            .flat_map(|(_, _, hosts)| hosts.iter().flatten())
            .map(|host| {
                HostPattern::parse(&format!("host-{host}.example"))
                    .expect("generated host is valid")
            })
            .collect::<Vec<_>>();
        let layers = requests
            .iter()
            .map(|(default_deny, requested_schemes, requested_hosts)| {
                let mut fields = Map::from_iter([
                    ("fetch_block_host".to_owned(), json!(["blocked.example"])),
                    ("trust_project_fetch_policy".to_owned(), json!(true)),
                ]);
                if let Some(value) = default_deny {
                    fields.insert("fetch_default_deny".to_owned(), json!(value));
                }
                if let Some(scheme_grants) = requested_schemes {
                    fields.insert("fetch_allow_scheme".to_owned(), json!(scheme_grants));
                }
                if let Some(host_grants) = requested_hosts {
                    let names = host_grants
                        .iter()
                        .map(|host| format!("host-{host}.example"))
                        .collect::<Vec<_>>();
                    fields.insert("fetch_allow_host".to_owned(), json!(names));
                }
                MergeLayer::file(Cow::Owned(Value::Object(fields)), None)
            })
            .collect::<Vec<_>>();

        let resolved = retain_layers_and_resolve_json(scope_project_chain(
            layers,
            requests.len() - 1,
        ));
        prop_assert!(resolved.errors.is_empty(), "generated policy fields are valid");
        let request = resolved
            .project_request
            .expect("a project occurrence yields a policy request");
        prop_assert_eq!(request.default_deny, expected_default_deny);
        prop_assert_eq!(request.allow_scheme, expected_schemes);
        prop_assert_eq!(request.allow_host, expected_hosts);

        for layer in resolved.layers {
            let value = layer.into_value();
            prop_assert!(value.get("fetch_default_deny").is_none());
            prop_assert!(value.get("fetch_allow_scheme").is_none());
            prop_assert!(value.get("fetch_allow_host").is_none());
            prop_assert!(value.get("trust_project_fetch_policy").is_none());
            prop_assert_eq!(
                value.get("fetch_block_host"),
                Some(&json!(["blocked.example"]))
            );
        }
    }

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
