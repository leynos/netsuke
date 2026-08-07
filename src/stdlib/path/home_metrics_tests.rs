//! Tests for the bounded home-resolution counter.
//!
//! Split from `home_tests`, which covers the ladders and the tracing events;
//! these cases pin the metric alone. A local `DebuggingRecorder` captures the
//! samples without touching the global recorder, so they stay as isolated as
//! the ladder cases. Both labels are drawn from closed sets in `path_utils`,
//! so the assertions below pin the series a dashboard would group by.

use super::path_utils::{EXPANDUSER_HOME_TOTAL, expanduser};
use crate::stdlib::config_types::HomeDirectory;
use metrics_util::MetricKind;
use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use rstest::rstest;

/// Resolve one path under a local recorder and return the counter samples
/// as `(outcome, source, value)` triples.
fn samples_for(
    home: &HomeDirectory,
    read_env: impl Fn(&str) -> Option<String>,
) -> Vec<(String, String, u64)> {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || {
        drop(expanduser("~/x", home, read_env));
    });
    snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .filter_map(|(key, _unit, _description, value)| {
            if key.kind() != MetricKind::Counter || key.key().name() != EXPANDUSER_HOME_TOTAL {
                return None;
            }
            let label = |name: &str| {
                key.key()
                    .labels()
                    .find(|label| label.key() == name)
                    .map(|label| label.value().to_owned())
            };
            let DebugValue::Counter(count) = value else {
                return None;
            };
            Some((label("outcome")?, label("source")?, count))
        })
        .collect()
}

/// A resolution that finds a home records `found` against the rung that
/// supplied it — one sample, whichever branch resolved it.
#[rstest]
#[case::explicit(HomeDirectory::Explicit("/home/a".to_owned()), "explicit")]
#[case::ambient(HomeDirectory::Ambient, "home")]
fn a_resolved_home_is_counted_against_its_source(
    #[case] home: HomeDirectory,
    #[case] expected_source: &str,
) {
    let read_env = |key: &str| (key == "HOME").then(|| "/home/a".to_owned());
    let samples = samples_for(&home, read_env);
    assert_eq!(
        samples,
        vec![("found".to_owned(), expected_source.to_owned(), 1)],
        "one sample labelled by outcome and source",
    );
}

/// A resolution that finds nothing records the failure outcome against the
/// `missing` source, and still counts exactly once despite the second debug
/// event the failure path emits.
#[rstest]
#[case::missing(HomeDirectory::Missing)]
#[case::ambient_with_an_empty_reader(HomeDirectory::Ambient)]
fn an_unresolvable_home_is_counted_once(#[case] home: HomeDirectory) {
    let samples = samples_for(&home, |_| None);
    assert_eq!(
        samples,
        vec![("home_unavailable".to_owned(), "missing".to_owned(), 1)],
        "the failure path adds an event, not a second sample",
    );
}
