//! Streaming regressions for the `netsuke help targets` renderers.
//!
//! `build_catalogue` emits one row per name, and every row of a name list
//! shares the target's description. The manifest budgets charge that
//! description once and count the target rather than its names, so nothing
//! else bounds the rendered catalogue. These regressions pin that both
//! renderers write incrementally, which keeps peak memory proportional to the
//! widest row rather than to the amplified total.

use super::bounds::manifest_with_targets;
use super::*;
use std::io::Write;

/// Bytes in the description shared by every name in these regressions.
const SHARED_DESCRIPTION_BYTES: usize = 16 * 1024;

/// Names that share that one description.
const SHARED_DESCRIPTION_NAMES: usize = 2_000;

/// Allowance for the two-space prefix, separators, a newline, and markers.
const ROW_OVERHEAD_BYTES: usize = 160;

/// Record the shape of a renderer's writes without retaining the bytes.
///
/// Retaining them would recreate the accumulated allocation under test. Before
/// the catalogue was streamed, the whole document arrived in a single write, so
/// `max_chunk` equalled `total_bytes`; bounding the two separately is what
/// distinguishes per-row streaming from an accumulated buffer.
#[derive(Default)]
struct ChunkStats {
    /// Bytes accepted across every write.
    total_bytes: usize,
    /// Bytes in the largest single write.
    max_chunk: usize,
    /// Number of writes performed.
    writes: usize,
}

impl Write for ChunkStats {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.total_bytes += buf.len();
        self.max_chunk = self.max_chunk.max(buf.len());
        self.writes += 1;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Build a manifest with one target whose names all share a large description.
fn shared_description_manifest() -> NetsukeManifest {
    let names = (0..SHARED_DESCRIPTION_NAMES)
        .map(|index| format!("shared{index:04}"))
        .collect();
    manifest_with_targets(vec![catalogue_target(
        names,
        Some("D".repeat(SHARED_DESCRIPTION_BYTES)),
        false,
        false,
    )])
}

/// The largest write one streamed row may produce.
fn one_row_bound() -> usize {
    SHARED_DESCRIPTION_BYTES + MAX_HELP_ALIGNMENT_WIDTH + ROW_OVERHEAD_BYTES
}

/// Assert the amplified catalogue arrived as many writes, none row-sized.
fn assert_streamed(sink: &ChunkStats, rows: usize) -> Result<()> {
    let amplified = rows * SHARED_DESCRIPTION_BYTES;
    anyhow::ensure!(
        sink.total_bytes >= amplified,
        "the catalogue should emit at least the amplified {amplified} bytes, saw {}",
        sink.total_bytes
    );
    anyhow::ensure!(
        sink.writes >= rows,
        "each of the {rows} rows should be written as it is rendered, saw {} writes",
        sink.writes
    );
    let bound = one_row_bound();
    anyhow::ensure!(
        sink.max_chunk <= bound,
        "largest write of {} bytes exceeds the one-row bound {bound}",
        sink.max_chunk
    );
    Ok(())
}

/// Guard text amplification: 2,000 names sharing one 16 KiB description must
/// be streamed one row at a time.
#[test]
fn text_catalogue_streams_rows_for_one_shared_large_description() -> Result<()> {
    let manifest = shared_description_manifest();
    let entries = build_catalogue(&manifest);
    let mut sink = ChunkStats::default();

    render_text(&mut sink, &entries, theme_prefs(ThemePreference::Unicode))?;

    assert_streamed(&sink, SHARED_DESCRIPTION_NAMES)
}

/// Guard JSON amplification: the same catalogue must be serialized
/// incrementally rather than formatted into one string first.
#[test]
fn json_catalogue_streams_entries_for_one_shared_large_description() -> Result<()> {
    let manifest = shared_description_manifest();
    let mut sink = ChunkStats::default();

    render_json(&mut sink, &build_catalogue(&manifest), None)?;

    assert_streamed(&sink, SHARED_DESCRIPTION_NAMES)
}
