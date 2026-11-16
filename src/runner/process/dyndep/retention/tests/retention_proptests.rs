//! Property tests for bounded sidecar retention.
//!
//! Retention is a lexically greedy selection under two independent budgets,
//! driven from a directory that promises no entry order. The named tests in
//! `budget.rs` pin single examples; these properties cover the space those
//! examples sample, so a change that keeps the examples and breaks the bound
//! still fails here.
//!
//! Selection is deliberately order-sensitive. It keeps at most
//! `max_files + 1` candidates so a large directory costs bounded memory, which
//! means a path reclaimed before a lexically earlier one arrives cannot be
//! reconsidered. The properties therefore separate the two guarantees that do
//! hold: arrivals in lexical order agree with an unbounded whole-set oracle,
//! and arrivals in any order stay inside both budgets without losing or
//! double-counting a candidate.
//!
//! The module name is load-bearing twice over. Its `proptests` suffix is what
//! `make proptest` selects on: that target filters by test-name segment, so a
//! property outside such a module never runs in the focused loop. Its
//! `retention_` stem keeps it clear of the `budget` prefix group that
//! `tests/workflow_contracts/rust_module_layout_test.py` polices, which rejects
//! a prefixed sibling such as `budget_proptests.rs` beside `budget.rs`.

use super::*;
use proptest::collection::btree_map;
use proptest::prelude::*;

/// Select lexically, accepting each candidate that still fits both budgets.
///
/// This is the property's independent oracle: it reads a whole candidate set at
/// once, whereas [`RetentionSelection`] sees one entry at a time.
fn reference_selection(
    candidates: &BTreeMap<String, u64>,
    policy: RetentionPolicy,
) -> BTreeMap<String, u64> {
    let mut retained = BTreeMap::new();
    let mut retained_bytes = 0_u64;
    for (name, bytes) in candidates {
        let has_file_capacity = retained.len() < policy.max_files;
        let has_byte_capacity = *bytes <= policy.max_bytes.saturating_sub(retained_bytes);
        if has_file_capacity && has_byte_capacity {
            retained_bytes = retained_bytes.saturating_add(*bytes);
            retained.insert(name.clone(), *bytes);
        }
    }
    retained
}

/// Convert an oracle result into the path-keyed shape selection state uses.
fn as_paths(retained: &BTreeMap<String, u64>) -> BTreeMap<Utf8PathBuf, u64> {
    retained
        .iter()
        .map(|(name, bytes)| (Utf8PathBuf::from(name.clone()), *bytes))
        .collect()
}

/// Run one selection over `candidates` in the given arrival order.
fn select_in_order<'a>(
    candidates: impl Iterator<Item = (&'a String, &'a u64)>,
    policy: RetentionPolicy,
) -> (BTreeMap<Utf8PathBuf, u64>, Vec<String>) {
    let mut selection = RetentionSelection::new(policy);
    let mut reclaimed = Vec::new();
    for (name, bytes) in candidates {
        let removed = selection.select(Utf8PathBuf::from(name.clone()), *bytes);
        reclaimed.extend(removed.into_iter().map(|(path, _)| path.to_string()));
    }
    (selection.paths, reclaimed)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Lexically ascending arrivals agree with the whole-set oracle.
    ///
    /// Ascending order is the case where bounded selection can be exact: every
    /// path that arrives later is lexically later, so none of them can displace
    /// a retained path. Agreement with an oracle that keeps the entire
    /// candidate set is therefore the strongest statement available, and it
    /// ties the budget arithmetic to an independently written rule.
    #[test]
    fn ascending_arrivals_match_the_whole_set_greedy_selection(
        candidates in btree_map("[a-z]{1,4}\\.dd", 0u64..=40, 0..12),
        max_files in 0usize..=4,
        max_bytes in 0u64..=80,
    ) {
        let policy = RetentionPolicy::new(max_files, max_bytes);
        let (retained, _) = select_in_order(candidates.iter(), policy);

        let expected = reference_selection(&candidates, policy);
        prop_assert_eq!(
            &retained,
            &as_paths(&expected),
            "ascending arrivals must reproduce the whole-set greedy result"
        );
        let retained_bytes: u64 = retained.values().sum();
        prop_assert!(
            retained.len() <= max_files,
            "retained {} paths above the {max_files}-file budget",
            retained.len()
        );
        prop_assert!(
            retained_bytes <= max_bytes,
            "retained {retained_bytes} bytes above the {max_bytes}-byte budget"
        );
    }

    /// Any arrival order stays inside both budgets and accounts for every
    /// candidate exactly once.
    ///
    /// Reverse arrival is the order most likely to break a selection that keeps
    /// only what it has already accepted, so it exercises the bounded path.
    #[test]
    fn descending_arrivals_stay_bounded_and_account_for_every_candidate(
        candidates in btree_map("[a-z]{1,4}\\.dd", 0u64..=40, 0..12),
        max_files in 0usize..=4,
        max_bytes in 0u64..=80,
    ) {
        let policy = RetentionPolicy::new(max_files, max_bytes);
        let (retained, reclaimed) = select_in_order(candidates.iter().rev(), policy);

        let retained_bytes: u64 = retained.values().sum();
        prop_assert!(
            retained.len() <= max_files,
            "retained {} paths above the {max_files}-file budget",
            retained.len()
        );
        prop_assert!(
            retained_bytes <= max_bytes,
            "retained {retained_bytes} bytes above the {max_bytes}-byte budget"
        );
        // Descent can reclaim a path it later wishes it had kept, so the
        // assertion here is accounting rather than an oracle comparison.
        let retained_names: Vec<String> = retained.keys().map(ToString::to_string).collect();

        let mut accounted = retained_names.clone();
        accounted.extend(reclaimed);
        accounted.sort();
        let mut all: Vec<String> = candidates.keys().cloned().collect();
        all.sort();
        prop_assert_eq!(
            accounted, all,
            "every candidate must be retained or reclaimed exactly once"
        );
    }

    /// Repeated publication with generated sizes and counts honours both
    /// budgets once the current bundle is set aside.
    #[test]
    fn repeated_publication_stays_within_both_budgets(
        content_len in 1usize..=24,
        publications in 1usize..=5,
        max_files in 0usize..=3,
        max_bytes in 0u64..=40,
    ) {
        let workspace = tempfile::tempdir().expect("the property needs a scratch workspace");
        let dir = temporary_dir(&workspace).expect("the scratch workspace must open as a capability");
        let policy = RetentionPolicy::new(max_files, max_bytes);
        let content = "x".repeat(content_len);
        let series = SidecarSeries::new("generated", &content, publications);
        let latest = publish_repeated_sidecars(&dir, series, policy)
            .expect("publishing within the property's bounds must succeed");

        let sidecars = sidecar_names(&dir)
            .expect("the scratch directory must be readable")
            .into_iter()
            .filter(|path| has_extension(path, "dd"))
            .count();
        prop_assert!(
            sidecars <= max_files + 1,
            "retained {sidecars} sidecars beyond the {max_files}-file budget and current bundle"
        );
        let total_bytes = retained_sidecar_bytes(&dir)
            .expect("the scratch directory must be readable");
        let obsolete_bytes = total_bytes
            .checked_sub(latest.content().len() as u64)
            .expect("retention must keep the current bundle's sidecar");
        prop_assert!(
            obsolete_bytes <= max_bytes,
            "retained {obsolete_bytes} obsolete bytes above the {max_bytes}-byte budget"
        );
        prop_assert!(
            dir.open(latest.relative_path()).is_ok(),
            "retention must preserve the latest bundle's sidecar"
        );
    }
}
