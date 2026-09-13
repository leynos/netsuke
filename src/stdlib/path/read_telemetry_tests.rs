//! Tests for the bounded file-read counter and its debug event.
//!
//! A local `DebuggingRecorder` captures the samples without touching the
//! global recorder, and every render below goes through the registered
//! filters rather than through `record_file_read` directly: that is what pins
//! each call site to its own filter label. Both labels are drawn from closed
//! sets in `read_telemetry`, so these cases assert the series a dashboard
//! would group by, and the event cases assert that nothing beyond those
//! bounded facts — least of all the path — leaves the process.

use anyhow::{Context, Result, anyhow, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use metrics_util::MetricKind;
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};
use minijinja::{Environment, context};
use rstest::rstest;
use tempfile::TempDir;
use tracing_subscriber::filter::LevelFilter;

use super::filters;
use super::read_telemetry::{
    FILE_READ_EVENT, FILE_READ_TOTAL, FILTER_CONTENTS, FILTER_DIGEST, FILTER_HASH,
    FILTER_LINECOUNT, OUTCOME_OK, OUTCOME_REJECTED,
};
use crate::stdlib::config_types::HomeDirectory;
use crate::test_tracing_capture::with_test_subscriber;

/// Name of the fixture file staged inside the temporary directory.
const FIXTURE_NAME: &str = "payload";

/// Bytes the fixture holds: inside a 1024-byte budget, over a 1-byte one.
const PAYLOAD: &[u8] = b"data";

/// Write `payload` to a temporary file and return the directory guard
/// alongside the file's path.
///
/// The write goes through a `cap_std` directory capability rather than
/// ambient `std::fs`, matching the convention the sibling test modules
/// follow. The guard must outlive the returned path: dropping it removes
/// the file.
fn fixture(payload: &[u8]) -> Result<(TempDir, Utf8PathBuf)> {
    let dir = tempfile::tempdir()?;
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
        .map_err(|path| anyhow!("temporary path is not valid UTF-8: {path:?}"))?;
    let handle = Dir::open_ambient_dir(&root, ambient_authority())?;
    handle.write(FIXTURE_NAME, payload)?;
    Ok((dir, root.join(FIXTURE_NAME)))
}

/// What one recorded filter call produced.
struct CallRecord {
    /// Whether the render produced a value.
    rendered: bool,
    /// `(filter, outcome, count)` samples drawn from the local recorder.
    samples: Vec<(String, String, u64)>,
}

/// Render `template` against `path` under a `limit`-byte budget, capturing the
/// counter locally.
fn call(path: &Utf8Path, limit: u64, template: &str) -> CallRecord {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let mut rendered = false;
    metrics::with_local_recorder(&recorder, || {
        let mut env = Environment::new();
        filters::register_filters(&mut env, HomeDirectory::Missing, limit);
        rendered = env
            .render_str(template, context!(path => path.as_str()))
            .is_ok();
    });
    CallRecord {
        rendered,
        samples: samples_of(&snapshotter),
    }
}

/// Extract this counter's `(filter, outcome, count)` samples from `snapshotter`.
fn samples_of(snapshotter: &Snapshotter) -> Vec<(String, String, u64)> {
    snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .filter_map(|(key, _unit, _description, value)| {
            if key.kind() != MetricKind::Counter || key.key().name() != FILE_READ_TOTAL {
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
            Some((label("filter")?, label("outcome")?, count))
        })
        .collect()
}

/// A call that renders records one sample under the filter's own label.
#[rstest]
#[case::contents(FILTER_CONTENTS, "{{ path | contents }}")]
#[case::linecount(FILTER_LINECOUNT, "{{ path | linecount }}")]
#[case::hash(FILTER_HASH, "{{ path | hash('sha256') }}")]
#[case::digest(FILTER_DIGEST, "{{ path | digest(8, 'sha256') }}")]
fn a_rendered_call_is_counted_as_ok(#[case] filter: &str, #[case] template: &str) -> Result<()> {
    let (_temp, file) = fixture(PAYLOAD)?;
    let record = call(&file, 1024, template);
    ensure!(
        record.rendered,
        "{filter}: the fixture must fit a 1024-byte budget"
    );
    ensure!(
        record.samples == vec![(filter.to_owned(), OUTCOME_OK.to_owned(), 1)],
        "{filter}: expected one ok sample but recorded {:?}",
        record.samples
    );
    Ok(())
}

/// A call the byte budget rejects records one sample under the filter's own
/// label — the budget is shared, but the filter that hit it is not.
#[rstest]
#[case::contents(FILTER_CONTENTS, "{{ path | contents }}")]
#[case::linecount(FILTER_LINECOUNT, "{{ path | linecount }}")]
#[case::hash(FILTER_HASH, "{{ path | hash('sha256') }}")]
#[case::digest(FILTER_DIGEST, "{{ path | digest(8, 'sha256') }}")]
fn a_rejected_call_is_counted_as_rejected(
    #[case] filter: &str,
    #[case] template: &str,
) -> Result<()> {
    let (_temp, file) = fixture(PAYLOAD)?;
    let record = call(&file, 1, template);
    ensure!(
        !record.rendered,
        "{filter}: a 4-byte file must exceed a 1-byte budget"
    );
    ensure!(
        record.samples == vec![(filter.to_owned(), OUTCOME_REJECTED.to_owned(), 1)],
        "{filter}: expected one rejected sample but recorded {:?}",
        record.samples
    );
    Ok(())
}

/// The debug event carries the bounded facts and nothing else.
///
/// The path is the fact that must never be recorded: it names a file on the
/// operator's host, so it is asserted absent from every captured event rather
/// than merely unasserted.
#[rstest]
#[case::rejected(1, OUTCOME_REJECTED)]
#[case::ok(1024, OUTCOME_OK)]
fn the_debug_event_is_bounded(#[case] limit: u64, #[case] outcome: &str) -> Result<()> {
    let (_temp, file) = fixture(PAYLOAD)?;
    let (rendered, captured) = with_test_subscriber(LevelFilter::DEBUG, |events| {
        let record = call(&file, limit, "{{ path | contents }}");
        (record.rendered, events.snapshot())
    });
    ensure!(
        rendered == (outcome == OUTCOME_OK),
        "a {limit}-byte budget should render the fixture only when it fits"
    );
    let read_events: Vec<&String> = captured
        .iter()
        .filter(|event| event.contains(FILE_READ_EVENT))
        .collect();
    ensure!(
        read_events.len() == 1,
        "expected one read event but captured {read_events:?}"
    );
    let event = read_events
        .first()
        .copied()
        .context("the length check above leaves one event")?;
    ensure!(
        event.contains("filter=\"contents\"")
            && event.contains(&format!("outcome=\"{outcome}\""))
            && event.contains(&format!("limit={limit}"))
            && event.contains("follow_symlinks=false"),
        "the event should carry the bounded facts: {event}"
    );
    ensure!(
        captured.iter().all(|other| !other.contains(file.as_str())),
        "no captured event may name the path it read: {captured:?}"
    );
    Ok(())
}
