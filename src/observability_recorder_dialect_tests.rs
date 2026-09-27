//! Verify the bounded recipe-text dialect counter series.
//!
//! The series this file guards is the one whose failure mode is invisible: an
//! unadmitted name makes `register_counter` return a `Counter::noop` handle, so
//! the counter records nothing while the build, the lint, and every other test
//! still pass. The first test therefore drives the *real* filter through the
//! recorder and asserts the increments arrive, rather than recording the series
//! by hand — a hand-recorded series would pass even if the filter never called
//! the recorder at all.

use super::{ConfigMetricsRecorder, SnapshotEntry};
use anyhow::Result;
use metrics::counter;
use metrics_util::{MetricKind, debugging::DebugValue};
use netsuke::{
    recipe_shell::RecipeShell,
    stdlib::{DIALECT_SOURCE_VALUES, DIALECT_VALUES, SHELL_QUOTE_DIALECT_TOTAL, StdlibConfig},
};
use rstest::rstest;

/// Render `template` with the recipe shell set to `shell` under a local
/// recorder, and return the snapshot.
///
/// The `Environment` is registered through the same `register_with_config`
/// entry point a real render uses, so the default dialect under test is the one
/// a manifest would actually receive rather than one this test chose.
///
/// Two bindings are supplied because the two filters under test take different
/// subject shapes: `shell_quote` encodes one string, while `shell_join`
/// requires a sequence. Binding only the string would leave `shell_join`
/// unrunnable here, and binding only the sequence would leave `shell_quote`
/// failing its own subject check.
///
/// The three fallible steps are propagated rather than unwrapped here because
/// this helper is not itself a test, and the workspace denies a panic outside
/// test-only code. Each caller unwraps at its own call site, inside a function
/// the lint recognizes.
fn recorded_render(shell: RecipeShell, template: &str) -> Result<Vec<SnapshotEntry>> {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();
    let config = StdlibConfig::from_current_dir()?.with_recipe_shell(shell);
    let mut env = minijinja::Environment::new();
    netsuke::stdlib::register_with_config(&mut env, config)?;

    metrics::with_local_recorder(&recorder, || {
        env.render_str(
            template,
            minijinja::context! { value => "a b", items => ["a", "b"] },
        )
    })?;

    Ok(snapshotter.snapshot().into_vec())
}

/// Count the retained increments for one `dialect`/`source` pair.
fn retained_count(snapshot: &[SnapshotEntry], dialect: &str, source: &str) -> u64 {
    snapshot
        .iter()
        .find_map(|entry| {
            if entry.0.kind() != MetricKind::Counter
                || entry.0.key().name() != SHELL_QUOTE_DIALECT_TOTAL
            {
                return None;
            }
            let labels: Vec<_> = entry.0.key().labels().collect();
            let matches = labels.len() == 2
                && labels
                    .iter()
                    .any(|label| label.key() == "dialect" && label.value() == dialect)
                && labels
                    .iter()
                    .any(|label| label.key() == "source" && label.value() == source);
            match (matches, &entry.3) {
                (true, DebugValue::Counter(count)) => Some(*count),
                _ => None,
            }
        })
        .unwrap_or(0)
}

/// Every retained series carries exactly the two bounded labels.
fn every_series_is_bounded(snapshot: &[SnapshotEntry]) -> bool {
    snapshot.iter().all(|entry| {
        let labels: Vec<_> = entry.0.key().labels().collect();
        labels.len() == 2
            && labels
                .iter()
                .all(|label| matches!(label.key(), "dialect" | "source"))
    })
}

/// The `(dialect, source)` pair one retained series carries, if it has both.
///
/// Extracted from the caller so the label walk is not nested inside the
/// generator loops that drive it.
fn dialect_pair(entry: &SnapshotEntry) -> Option<(String, String)> {
    let labels: Vec<_> = entry.0.key().labels().collect();
    let value_of = |wanted: &str| {
        labels
            .iter()
            .find(|label| label.key() == wanted)
            .map(|label| label.value().to_owned())
    };
    Some((value_of("dialect")?, value_of("source")?))
}

/// The filter's own resolutions reach the recorder and are retained.
///
/// This is the constraint-13 test. An unadmitted name produces a noop handle,
/// so both assertions below fail together while nothing else in the suite
/// notices: the render still succeeds and the snapshot is simply empty.
#[rstest]
#[case::omitted_under_posix(RecipeShell::Posix, "sh", "default")]
#[case::omitted_under_power_shell(RecipeShell::PowerShell, "powershell", "default")]
#[case::explicit_sh_under_power_shell(RecipeShell::PowerShell, "sh", "explicit")]
#[case::explicit_power_shell_under_posix(RecipeShell::Posix, "powershell", "explicit")]
fn a_filter_resolution_reaches_the_recorder(
    #[case] shell: RecipeShell,
    #[case] dialect: &str,
    #[case] expected_source: &str,
) {
    let template = if expected_source == "default" {
        "{{ value | shell_quote }}".to_owned()
    } else {
        format!("{{{{ value | shell_quote(dialect='{dialect}') }}}}")
    };

    let snapshot =
        recorded_render(shell, &template).expect("the filter renders under the test host");

    assert_eq!(
        retained_count(&snapshot, dialect, expected_source),
        1,
        "one resolution of {dialect}/{expected_source} should be retained: {snapshot:?}"
    );
    assert!(
        every_series_is_bounded(&snapshot),
        "only bounded dialect series are retained: {snapshot:?}"
    );
}

/// `shell_join` resolves the dialect at the same boundary and is counted there.
#[test]
fn shell_join_records_at_the_same_boundary() {
    let snapshot = recorded_render(RecipeShell::Posix, "{{ items | shell_join }}")
        .expect("the filter renders under the test host");

    assert_eq!(
        retained_count(&snapshot, "sh", "default"),
        1,
        "shell_join resolves through resolve_dialect too: {snapshot:?}"
    );
}

/// Every dialect is counted once, and no other series survives.
///
/// The four combinations are the whole label space, so a series outside them is
/// a defect in the admission sets rather than a missing case.
#[test]
fn recorder_retains_only_the_bounded_dialect_series() {
    let recorder = ConfigMetricsRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        for dialect in DIALECT_VALUES {
            for source in DIALECT_SOURCE_VALUES {
                counter!(SHELL_QUOTE_DIALECT_TOTAL, "dialect" => dialect, "source" => source)
                    .increment(1);
            }
        }
        counter!(SHELL_QUOTE_DIALECT_TOTAL, "dialect" => "unbounded", "source" => "default")
            .increment(1);
        counter!(SHELL_QUOTE_DIALECT_TOTAL, "dialect" => "sh", "source" => "unbounded")
            .increment(1);
        counter!(SHELL_QUOTE_DIALECT_TOTAL, "dialect" => "sh").increment(1);
        counter!(SHELL_QUOTE_DIALECT_TOTAL).increment(1);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        snapshot.len(),
        DIALECT_VALUES.len() * DIALECT_SOURCE_VALUES.len(),
        "only the bounded dialect/source combinations are retained"
    );
    assert!(every_series_is_bounded(&snapshot), "{snapshot:?}");
}

/// The admitted vocabulary is exactly the vocabulary the filter emits.
///
/// The recording path is driven rather than handwritten, so this fails if the
/// filter ever emits a label the admission sets do not carry — which is the
/// silent-noop defect — rather than merely asserting the constants agree with
/// themselves, which they always would.
#[test]
fn every_emitted_dialect_source_pair_is_admitted() {
    let mut emitted = std::collections::BTreeSet::new();
    for shell in [RecipeShell::Posix, RecipeShell::PowerShell] {
        for template in [
            "{{ value | shell_quote }}",
            "{{ value | shell_quote(dialect='sh') }}",
            "{{ value | shell_quote(dialect='powershell') }}",
            "{{ items | shell_join(dialect='sh') }}",
        ] {
            let dialect_series = recorded_render(shell, template)
                .expect("the filter renders under the test host")
                .into_iter()
                .filter(|entry| entry.0.key().name() == SHELL_QUOTE_DIALECT_TOTAL)
                .filter_map(|entry| dialect_pair(&entry));
            emitted.extend(dialect_series);
        }
    }

    let admitted: std::collections::BTreeSet<_> = DIALECT_VALUES
        .iter()
        .flat_map(|dialect| {
            DIALECT_SOURCE_VALUES
                .iter()
                .map(move |source| ((*dialect).to_owned(), (*source).to_owned()))
        })
        .collect();

    assert!(
        emitted.is_subset(&admitted),
        "every emitted pair must be admitted: emitted={emitted:?} admitted={admitted:?}"
    );
    assert!(
        !emitted.is_empty(),
        "the recording path must emit at least one pair, or the subset check is vacuous"
    );
}
