//! The counts RFC 0006 states about itself.
//!
//! Two of them are consumed by the coverage checks. Table 11 tallies the survey
//! — how many rows fell into each reject class, how many new filters and tests
//! the RFC introduces, how many existing helpers gain an option — and section
//! 6.1's purity sentence aggregates the accepted set into pure,
//! filesystem-observing, and environment-observing.
//!
//! Section 6.1's numbers are number words rather than digits, so they cannot be
//! parsed without a number-word table. They are transcribed and then guarded by
//! asserting the sentence they came from is still present verbatim.

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use super::Section;

/// Section 6.1's purity aggregate, transcribed from words rather than digits.
///
/// The sentence reads "Of the fifty-seven helpers proposed here, fifty-two are
/// pure, four are filesystem-observing, and one is environment-observing." The
/// numbers are words, so they cannot be parsed without a number-word table; the
/// transcription is guarded by [`purity_aggregate`] asserting the sentence is
/// still present verbatim, whitespace collapsed.
const PURITY: (usize, usize, usize) = (52, 4, 1);

/// The helper count section 6.1's aggregate ranges over.
pub(super) const PROPOSED_HELPERS: usize = 57;

/// The verbatim purity sentence [`PURITY`] transcribes, whitespace collapsed.
const PURITY_SENTENCE: &str = "Of the fifty-seven helpers proposed here, fifty-two are pure, \
four are filesystem-observing, and one is environment-observing.";

/// The tally rows of RFC 0006 table 11 that this test consumes.
pub(super) struct Totals {
    /// Reject rows classed "already provides", "redundant alias", "principle".
    pub(super) reject_classes: [usize; 3],
    /// New Netsuke filters table 11 counts.
    pub(super) new_filters: usize,
    /// New Netsuke tests table 11 counts.
    pub(super) new_tests: usize,
    /// Existing helpers gaining an option, per table 11.
    pub(super) optioned: usize,
}

/// Read table 11 by row label.
pub(super) fn table_11(section_7: &Section<'_>) -> Result<Totals> {
    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    for (heading, rows) in section_7.tables() {
        if !heading.starts_with("7.8.") {
            continue;
        }
        for row in &rows {
            let label = row.cell(0, "measure")?.to_owned();
            let count: usize = row
                .cell(1, "count")?
                .replace(',', "")
                .parse()
                .with_context(|| format!("table 11's count for {label:?} is not a number"))?;
            ensure!(
                totals.insert(label.clone(), count).is_none(),
                "table 11 states {label:?} twice"
            );
        }
    }
    let find = |prefix: &str| -> Result<usize> {
        let hits: Vec<(&String, &usize)> = totals
            .iter()
            .filter(|(label, _)| label.starts_with(prefix))
            .collect();
        ensure!(
            hits.len() == 1,
            "expected exactly one table 11 row starting {prefix:?}, found {:?}",
            hits.iter()
                .map(|(label, _)| label.as_str())
                .collect::<Vec<_>>()
        );
        hits.first().map(|(_, count)| **count).with_context(|| {
            format!("table 11's row starting {prefix:?} vanished between the check and the read")
        })
    };
    Ok(Totals {
        reject_classes: [
            find("Surveyed entries rejected because")?,
            find("Surveyed entries rejected as a redundant alias")?,
            find("Surveyed entries rejected on principle")?,
        ],
        new_filters: find("New Netsuke filters introduced")?,
        new_tests: find("New Netsuke tests introduced")?,
        optioned: find("Existing Netsuke helpers gaining")?,
    })
}

/// Section 6.1's purity aggregate, guarded by the sentence it is read from.
pub(super) fn purity_aggregate(document: &Section<'_>) -> Result<(usize, usize, usize)> {
    let section = document
        .subsection("### 6.1. Purity classes")
        .context("RFC 0006 has no section 6.1")?;
    let collapsed = section
        .lines
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    ensure!(
        collapsed.contains(PURITY_SENTENCE),
        "RFC 0006 section 6.1 no longer states the purity aggregate this test transcribes: \
         {PURITY_SENTENCE}"
    );
    Ok(PURITY)
}
