//! Property tests for bounded sidecar retention.
//!
//! Retention is a lexically greedy selection under two independent budgets,
//! driven from a directory that promises no entry order. The named tests in
//! `budget.rs` pin single examples; these properties cover the space those
//! examples sample, so a change that keeps the examples and breaks the bound
//! still fails here.
//!
//! ADR-0012 promises that obsolete sidecars are considered in deterministic
//! path order, so the retained set must not depend on the order the filesystem
//! happens to list the directory. The retention pass therefore sorts entry
//! names before selection sees them, and the permutation property below checks
//! that promise through the whole pruning path; a shuffled creation order is
//! the closest a test can come to an unpredictable listing.
//!
//! Selection state itself stays bounded: it keeps at most `max_files + 1`
//! candidates so a large directory costs bounded memory, and reapplies the
//! greedy rule over the retained set so a caller that supplies another order
//! still gets the right arithmetic. The ascending property checks that state
//! against an independently written whole-set oracle.
//!
//! The module name is load-bearing twice over. Its `proptests` suffix is what
//! `make proptest` selects on: that target filters by test-name segment, so a
//! property outside such a module never runs in the focused loop. Its
//! `retention_` stem keeps it clear of the `budget` prefix group that
//! `tests/workflow_contracts/rust_module_layout_test.py` polices, which rejects
//! a prefixed sibling such as `budget_proptests.rs` beside `budget.rs`.

use super::*;
use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use std::ops::RangeInclusive;

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

/// Order candidates by a seeded hash of their names.
///
/// The creation order must be unrelated to path order and reproducible from the
/// printed proptest seed, so a failing case can be replayed from it alone.
/// Sorting by a hash of each name mixed with the seed gives both without a
/// random number generator: the result is a permutation of the candidates, and
/// the retention pass is required not to observe it.
fn seed_ordered<'a>(candidates: &mut [(&'a String, &'a u64)], seed: u64) {
    candidates.sort_by_key(|(name, _)| hash(name.as_bytes()) ^ seed);
}

/// Hash `bytes` with FNV-1a, purely to scatter the creation order.
fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |digest, byte| {
        (digest ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Return whether `name` is a Windows reserved device name.
///
/// Windows resolves `con`, `prn`, `aux`, `nul`, `com1`–`com9` and `lpt1`–`lpt9`
/// to devices in every directory and whatever extension they carry, so
/// `prn.dd` cannot be created as a sidecar there even though it matches the
/// generated name shape. Such a name can never reach the retention pass on the
/// host it would fail on, so generating one only tests the filesystem.
fn is_windows_device_name(name: &str) -> bool {
    let stem = name.strip_suffix(".dd").unwrap_or(name);
    if matches!(stem, "con" | "prn" | "aux" | "nul") {
        return true;
    }
    ["com", "lpt"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|number| {
            // Only the single digits 1–9 name devices; `com0` and `com10` are
            // ordinary filenames.
            number.len() == 1 && number != "0" && number.bytes().all(|b| b.is_ascii_digit())
        })
    })
}

/// Generate candidate sidecars that a Windows host can actually create.
///
/// The properties materialise their candidates as files, so the generator
/// excludes names no host in the CI matrix can write. Filtering here rather
/// than in each property keeps the two candidate sets shaped alike.
fn writable_candidates(
    values: RangeInclusive<u64>,
) -> impl Strategy<Value = BTreeMap<String, u64>> {
    btree_map("[a-z]{1,4}\\.dd", values, 0..12).prop_filter(
        "generated sidecars must be creatable on Windows",
        |candidates| candidates.keys().all(|name| !is_windows_device_name(name)),
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Lexically ascending arrivals agree with the whole-set oracle.
    ///
    /// This ties the bounded selection state's arithmetic to an independently
    /// written rule over the same inputs.
    #[test]
    fn ascending_arrivals_match_the_whole_set_greedy_selection(
        candidates in writable_candidates(0..=40),
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

    /// The pass retains the greedy selection whatever order creation happened in.
    ///
    /// Everything is exercised through the pruning entry point rather than
    /// through [`RetentionSelection`] alone, because ordering is the pass's
    /// responsibility: a pass that selected as it read the directory would
    /// agree with this oracle only by luck of the listing order.
    #[test]
    fn shuffled_creation_order_yields_the_whole_set_greedy_selection(
        candidates in writable_candidates(1..=40),
        max_files in 0usize..=4,
        max_bytes in 0u64..=80,
        seed in any::<u64>(),
    ) {
        let workspace = tempfile::tempdir().expect("the property needs a scratch workspace");
        let dir = temporary_dir(&workspace)
            .expect("the scratch workspace must open as a capability");
        let policy = RetentionPolicy::new(max_files, max_bytes);

        let mut creation_order: Vec<(&String, &u64)> = candidates.iter().collect();
        seed_ordered(&mut creation_order, seed);
        for (name, bytes) in creation_order {
            let length = usize::try_from(*bytes)
                .expect("generated candidate sizes fit a usize");
            let published = sidecar(&format!(".netsuke/dyndep/{name}"), &"x".repeat(length));
            drop(
                materialize_dyndep_files(&dir, std::slice::from_ref(&published))
                    .expect("each candidate sidecar must materialize"),
            );
        }

        let current = sidecar(".netsuke/dyndep/current.dd", "current");
        let lease = materialize_dyndep_files(&dir, std::slice::from_ref(&current))
            .expect("the current bundle must materialize");
        prune_dyndep_sidecars(&dir, &lease, std::slice::from_ref(&current), policy)
            .expect("retention must succeed within the property's bounds");

        let retained: BTreeMap<String, u64> = sidecar_names(&dir)
            .expect("the scratch directory must be readable")
            .into_iter()
            .filter(|path| has_extension(path, "dd"))
            .filter_map(|path| {
                let name = dyndep_entry_name(&path)?.to_owned();
                if name == "current.dd" {
                    return None;
                }
                let bytes = dir
                    .metadata(&path)
                    .expect("a retained sidecar must be readable")
                    .len();
                Some((name, bytes))
            })
            .collect();

        prop_assert_eq!(
            &retained,
            &reference_selection(&candidates, policy),
            "the retained sidecars must not depend on the creation order"
        );
    }

    /// Repeated publication with generated, varying sizes honours both budgets
    /// once the current bundle is set aside.
    #[test]
    fn repeated_publication_stays_within_both_budgets(
        sizes in vec(1usize..=24, 1..=5),
        max_files in 0usize..=3,
        max_bytes in 0u64..=40,
    ) {
        let workspace = tempfile::tempdir().expect("the property needs a scratch workspace");
        let dir = temporary_dir(&workspace)
            .expect("the scratch workspace must open as a capability");
        let policy = RetentionPolicy::new(max_files, max_bytes);

        let mut latest = None;
        for (index, size) in sizes.iter().enumerate() {
            let published =
                sidecar(&format!(".netsuke/dyndep/sized-{index}.dd"), &"x".repeat(*size));
            let lease = materialize_dyndep_files(&dir, std::slice::from_ref(&published))
                .expect("a published sidecar must materialize");
            prune_dyndep_sidecars(&dir, &lease, std::slice::from_ref(&published), policy)
                .expect("retention must succeed within the property's bounds");
            latest = Some(published);
            drop(lease);
        }
        let latest_sidecar = latest.expect("a generated series is never empty");

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
            .checked_sub(latest_sidecar.content().len() as u64)
            .expect("retention must keep the current bundle's sidecar");
        prop_assert!(
            obsolete_bytes <= max_bytes,
            "retained {obsolete_bytes} obsolete bytes above the {max_bytes}-byte budget"
        );
        prop_assert!(
            dir.open(latest_sidecar.relative_path()).is_ok(),
            "retention must preserve the latest bundle's sidecar"
        );
    }
}

/// The candidate generator must never offer a name Windows cannot create.
///
/// A generated `prn.dd` reaches `materialize_dyndep_files` on the Windows CI
/// legs and fails to be written, which is a property of that host rather than
/// of retention. Pin the predicate and the strategy that uses it, so widening
/// the generated name shape fails here rather than on a Windows runner.
#[test]
fn generated_candidates_exclude_windows_device_names() {
    let device_names = [
        "con.dd", "prn.dd", "aux.dd", "nul.dd", "com1.dd", "com9.dd", "lpt1.dd", "lpt9.dd",
    ];
    for name in device_names {
        assert!(
            is_windows_device_name(name),
            "{name} is a Windows device name"
        );
    }
    let ordinary_names = ["com0.dd", "com10.dd", "printer.dd", "conx.dd"];
    for name in ordinary_names {
        assert!(
            !is_windows_device_name(name),
            "{name} is an ordinary sidecar name"
        );
    }
}

proptest! {
    /// The candidate generator never offers a name Windows cannot create.
    ///
    /// The predicate test above pins the classification; this checks that the
    /// strategy actually applies it, so a later widening of the generated name
    /// shape fails here instead of on a Windows runner.
    #[test]
    fn generated_candidates_are_writable_sidecars(candidates in writable_candidates(0..=40)) {
        prop_assert!(
            candidates.keys().all(|name| !is_windows_device_name(name)),
            "the generator offered a Windows device name: {candidates:?}"
        );
    }
}
