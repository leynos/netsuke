//! Verify the bounded `which` resolver counter series.
//!
//! The recorder admits each series by its exact label shape, so these cases
//! are the allowlist's own specification: a `cwd_mode` outside the closed
//! vocabulary, a missing outcome, an undeclared extra label, or the failure
//! `category` on a success series must all be refused rather than exported.

use super::{ConfigMetricsRecorder, SnapshotEntry};
use metrics::counter;
use metrics_util::{MetricKind, debugging::DebugValue};
use netsuke::stdlib::{WHICH_CACHE_TOTAL, WHICH_RESOLUTION_TOTAL};

/// Retain only the bounded `cwd_mode` and `outcome` pairs on cache counters.
#[test]
fn recorder_retains_bounded_which_cache_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        counter!(WHICH_CACHE_TOTAL, "cwd_mode" => "auto", "outcome" => "hit").increment(1);
        counter!(WHICH_CACHE_TOTAL, "cwd_mode" => "always", "outcome" => "miss").increment(2);
        counter!(WHICH_CACHE_TOTAL, "cwd_mode" => "never", "outcome" => "bypass").increment(3);
        counter!(
            WHICH_CACHE_TOTAL,
            "cwd_mode" => "workspace_recursive",
            "outcome" => "hit",
        )
        .increment(4);
        // A hyphenated spelling is the template's, not the label's.
        counter!(WHICH_CACHE_TOTAL, "cwd_mode" => "workspace-recursive", "outcome" => "hit")
            .increment(1);
        counter!(WHICH_CACHE_TOTAL, "cwd_mode" => "somedir", "outcome" => "hit").increment(1);
        counter!(WHICH_CACHE_TOTAL, "cwd_mode" => "auto", "outcome" => "unbounded").increment(1);
        counter!(WHICH_CACHE_TOTAL, "cwd_mode" => "auto").increment(1);
        counter!(WHICH_CACHE_TOTAL, "outcome" => "hit").increment(1);
        counter!(WHICH_CACHE_TOTAL, "cwd_mode" => "auto", "outcome" => "hit", "command" => "ls")
            .increment(1);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        4,
        "only the four bounded cache series are retained"
    );
    for (cwd_mode, outcome, count) in [
        ("auto", "hit", 1),
        ("always", "miss", 2),
        ("never", "bypass", 3),
        ("workspace_recursive", "hit", 4),
    ] {
        assert_cache_counter(&snapshot, cwd_mode, outcome, count);
    }
}

/// Record the valid and refused resolution series used by the shape cases.
///
/// Kept apart from the assertions so each function stays within the
/// repository's function-length bound.
fn record_mixed_resolution_series() {
    counter!(
        WHICH_RESOLUTION_TOTAL,
        "cwd_mode" => "workspace_recursive",
        "outcome" => "found",
    )
    .increment(7);
    counter!(
        WHICH_RESOLUTION_TOTAL,
        "cwd_mode" => "auto",
        "outcome" => "not_found",
        "category" => "not_found",
    )
    .increment(1);
    counter!(
        WHICH_RESOLUTION_TOTAL,
        "cwd_mode" => "never",
        "outcome" => "error",
        "category" => "walkdir",
    )
    .increment(2);
    // A failure category outside the closed set is refused.
    counter!(
        WHICH_RESOLUTION_TOTAL,
        "cwd_mode" => "auto",
        "outcome" => "error",
        "category" => "unbounded",
    )
    .increment(1);
    // A `cwd_mode` outside the closed set is refused on either shape.
    counter!(
        WHICH_RESOLUTION_TOTAL,
        "cwd_mode" => "somedir",
        "outcome" => "not_found",
        "category" => "not_found",
    )
    .increment(1);
    // A category the vocabulary declares, but the success shape may not
    // carry it: three labels only, and never on a found outcome.
    counter!(
        WHICH_RESOLUTION_TOTAL,
        "cwd_mode" => "auto",
        "outcome" => "found",
        "category" => "not_found",
    )
    .increment(1);
    // A declared third label set plus one undeclared extra label.
    counter!(
        WHICH_RESOLUTION_TOTAL,
        "cwd_mode" => "auto",
        "outcome" => "error",
        "category" => "not_found",
        "command" => "ls",
    )
    .increment(1);
    // The success shape with no `cwd_mode` at all.
    counter!(WHICH_RESOLUTION_TOTAL, "outcome" => "found").increment(1);
    // A failure recorded without its category. The two-label shape admits only
    // `found`, so this cannot be mistaken for a success series: the outcome is
    // not in that vocabulary and there is no `category` to reach the other.
    counter!(WHICH_RESOLUTION_TOTAL, "cwd_mode" => "auto", "outcome" => "not_found").increment(1);
    counter!(WHICH_RESOLUTION_TOTAL, "cwd_mode" => "never", "outcome" => "error").increment(1);
}

/// Admit the resolution counter under both of its label shapes.
///
/// A success carries `cwd_mode` and `outcome`; a failure adds `category`. The
/// counter is therefore the one series whose label count is not fixed, and
/// both shapes have to be admitted for its failures to reach the snapshot.
///
/// Each shape admits only its own outcomes. A `found` series carrying a
/// category and a failure written without one are both refused, so the two
/// shapes are complements rather than one being a superset of the other.
#[test]
fn recorder_retains_both_bounded_which_resolution_shapes() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, record_mixed_resolution_series);

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        3,
        "only the three bounded resolution series are retained"
    );
    assert_which_counter(&snapshot, &Series::found("workspace_recursive", 7));
    assert_which_counter(
        &snapshot,
        &Series::failed("auto", "not_found", "not_found", 1),
    );
    assert_which_counter(&snapshot, &Series::failed("never", "error", "walkdir", 2));
}

/// Every category the resolver can emit is admitted on a failure series.
///
/// The recorder carries its own copy of the category vocabulary, so this case
/// pins the two together: a category the resolver declares but the recorder
/// omits would drop a real failure from the snapshot.
#[test]
fn recorder_admits_every_declared_resolution_category() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();
    let categories = netsuke::stdlib::RESOLVE_ERROR_CATEGORY_VALUES;

    metrics::with_local_recorder(&recorder, || {
        for category in categories {
            counter!(
                WHICH_RESOLUTION_TOTAL,
                "cwd_mode" => "workspace_recursive",
                "outcome" => "error",
                "category" => category,
            )
            .increment(1);
        }
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        categories.len(),
        "every declared category should survive as its own series"
    );
    for category in categories {
        assert_which_counter(
            &snapshot,
            &Series::failed("workspace_recursive", "error", category, 1),
        );
    }
}

/// One expected `which` resolution series.
///
/// Grouping the expectations keeps the assertion helper within the
/// repository's argument bound, and names the two shapes the counter takes:
/// [`Series::found`] for the two-label success and [`Series::failed`] for the
/// three-label failure.
#[derive(Debug)]
struct Series<'a> {
    /// Expected `cwd_mode` label value.
    cwd_mode: &'a str,
    /// Expected `outcome` label value.
    outcome: &'a str,
    /// Expected `category` label value, absent on a success.
    category: Option<&'a str>,
    /// Expected counter value.
    count: u64,
}

impl<'a> Series<'a> {
    /// A success series: two labels, no category.
    const fn found(cwd_mode: &'a str, count: u64) -> Self {
        Self {
            cwd_mode,
            outcome: "found",
            category: None,
            count,
        }
    }

    /// A failure series: three labels, including the bounded category.
    const fn failed(cwd_mode: &'a str, outcome: &'a str, category: &'a str, count: u64) -> Self {
        Self {
            cwd_mode,
            outcome,
            category: Some(category),
            count,
        }
    }

    /// The number of labels this shape must carry.
    const fn label_count(&self) -> usize {
        if self.category.is_some() { 3 } else { 2 }
    }

    /// Whether `entry` is this series.
    fn matches(&self, entry: &SnapshotEntry) -> bool {
        if !Self::is_resolution_counter(entry) {
            return false;
        }
        let labels = LabelSet::of(entry);
        labels.label_count() == self.label_count()
            && labels.has("cwd_mode", self.cwd_mode)
            && labels.has("outcome", self.outcome)
            && self.category_matches(&labels)
            && self.count_matches(entry)
    }

    /// Whether `entry` is a counter under the resolution metric's own name.
    ///
    /// A success and a failure series differ only in their labels, so this is
    /// what keeps a case from matching some other metric's counter that
    /// happened to carry the labels it expected. The question is about the
    /// entry rather than about any expectation, so it is an associated
    /// function: there is nothing on `self` for it to read.
    fn is_resolution_counter(entry: &SnapshotEntry) -> bool {
        entry.0.kind() == MetricKind::Counter && entry.0.key().name() == WHICH_RESOLUTION_TOTAL
    }

    /// Whether `labels` carry the category this shape expects.
    ///
    /// A success is identified as much by the *absence* of a category as by
    /// the presence of the right one: a failure category left on a success
    /// series would otherwise satisfy the label count and go unremarked.
    fn category_matches(&self, labels: &LabelSet<'_>) -> bool {
        self.category.map_or_else(
            || !labels.names("category"),
            |value| labels.has("category", value),
        )
    }

    /// Whether `entry` retained the counter value this shape expects.
    fn count_matches(&self, entry: &SnapshotEntry) -> bool {
        matches!(entry.3, DebugValue::Counter(observed) if observed == self.count)
    }
}

/// The labels one retained series carries, borrowed from its metric key.
///
/// A series is identified by its label *set* rather than by any single label,
/// so every assertion in this file asks the same two questions of one: whether
/// it carries a given pair, and how many pairs it carries in total. Answering
/// them here keeps that shape in one place instead of repeating the scan at
/// each call site, and keeps each caller down to the one decision it is about.
struct LabelSet<'a> {
    /// The series' labels, borrowed from the key they were read off.
    labels: Vec<&'a metrics::Label>,
}

impl<'a> LabelSet<'a> {
    /// Read the labels off a retained entry.
    fn of(entry: &'a SnapshotEntry) -> Self {
        Self {
            labels: entry.0.key().labels().collect(),
        }
    }

    /// Whether the series carries `key` with exactly `value`.
    fn has(&self, key: &str, value: &str) -> bool {
        self.labels
            .iter()
            .any(|label| label.key() == key && label.value() == value)
    }

    /// Whether the series carries `key` under any value at all.
    fn names(&self, key: &str) -> bool {
        self.labels.iter().any(|label| label.key() == key)
    }

    /// How many labels the series carries.
    fn label_count(&self) -> usize {
        self.labels.len()
    }
}

/// Assert one retained `which` resolution counter matches `expected`.
fn assert_which_counter(snapshot: &[SnapshotEntry], expected: &Series<'_>) {
    assert!(
        snapshot.iter().any(|entry| expected.matches(entry)),
        "expected a retained {WHICH_RESOLUTION_TOTAL} series for {expected:?}: {snapshot:?}"
    );
}

/// Assert one retained `which` cache counter has `cwd_mode` and `outcome`.
fn assert_cache_counter(snapshot: &[SnapshotEntry], cwd_mode: &str, outcome: &str, expected: u64) {
    assert!(
        snapshot
            .iter()
            .any(|entry| is_cache_counter(entry, cwd_mode, outcome, expected)),
        "expected a retained {WHICH_CACHE_TOTAL} series for {cwd_mode}/{outcome}={expected}: {snapshot:?}"
    );
}

/// Whether `entry` is the cache series for `cwd_mode`/`outcome` and `expected`.
///
/// The pair is asserted as a *set of two*, not as two independent memberships:
/// a series carrying the right pair plus anything else has a shape the
/// recorder's allowlist does not admit, so matching it here would let a case
/// pass over a series the recorder should have refused.
fn is_cache_counter(entry: &SnapshotEntry, cwd_mode: &str, outcome: &str, expected: u64) -> bool {
    let labels = LabelSet::of(entry);
    entry.0.kind() == MetricKind::Counter
        && entry.0.key().name() == WHICH_CACHE_TOTAL
        && labels.label_count() == 2
        && labels.has("cwd_mode", cwd_mode)
        && labels.has("outcome", outcome)
        && matches!(entry.3, DebugValue::Counter(observed) if observed == expected)
}
