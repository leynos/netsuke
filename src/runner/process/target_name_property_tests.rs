//! Property tests for bounded target rendering and deterministic candidates.

use super::{
    CommandListTargetIndex, MAX_SHOWN_CANDIDATES, MAX_TARGET_NAME_BYTES, TRUNCATION_MARKER,
    bound_target_name,
};
use crate::runner::process::test_fixtures::{
    TEST_ACTION_ID, graph_with_identity_action, insert_edge,
};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn bounded_names_are_debug_escaped_prefixes(characters in prop::collection::vec(any::<char>(), 0..=128)) {
        let target = String::from_iter(characters);
        let full_debug_name = format!("{target:?}");
        let bounded_name = bound_target_name(&target);
        let full_contents = full_debug_name
            .strip_prefix('"')
            .and_then(|name| name.strip_suffix('"'))
            .expect("Rust string debug output should be quoted");
        let bounded_contents = bounded_name
            .strip_prefix('"')
            .and_then(|name| name.strip_suffix('"'))
            .expect("bounded names should be quoted");

        prop_assert!(bounded_name.len() <= MAX_TARGET_NAME_BYTES);
        prop_assert!(bounded_name.starts_with('"') && bounded_name.ends_with('"'));
        if full_debug_name.len() <= MAX_TARGET_NAME_BYTES {
            prop_assert_eq!(bounded_name, full_debug_name);
        } else {
            let retained_prefix = bounded_contents
                .strip_suffix(TRUNCATION_MARKER)
                .expect("long names should carry a visible truncation marker");
            prop_assert!(full_contents.starts_with(retained_prefix));
        }
    }

    #[test]
    fn candidate_order_and_remainder_are_stable(names in prop::collection::vec("[a-z]{1,12}", 1..=8)) {
        let mut unique_names = names;
        unique_names.sort();
        unique_names.dedup();
        let forward = candidate_clause(&unique_names);
        unique_names.reverse();
        let reversed = candidate_clause(&unique_names);

        prop_assert_eq!(forward.as_deref(), reversed.as_deref());
        let sorted_names = {
            let mut expected_names = unique_names;
            expected_names.sort();
            expected_names
        };
        let expected = match sorted_names.as_slice() {
            [target] => format!("target {target:?}"),
            candidates => {
                let shown = candidates
                    .iter()
                    .take(MAX_SHOWN_CANDIDATES)
                    .map(|target| format!("{target:?}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let remaining = candidates.len().saturating_sub(MAX_SHOWN_CANDIDATES);
                if remaining == 0 {
                    format!("targets {shown}")
                } else {
                    format!("targets {shown} and {remaining} more")
                }
            }
        };
        prop_assert_eq!(forward.as_deref(), Some(expected.as_str()));
    }
}

/// Resolve candidate names in their insertion order using the production index.
fn candidate_clause(names: &[String]) -> Option<String> {
    let mut graph = graph_with_identity_action();
    for name in names {
        insert_edge(&mut graph, TEST_ACTION_ID, &[name.as_str()], false);
    }
    CommandListTargetIndex::from_graph(&graph).clause(
        &crate::ninja_gen::command_list::action_identity(TEST_ACTION_ID),
    )
}
