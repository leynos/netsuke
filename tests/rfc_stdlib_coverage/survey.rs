//! Derivation of the accepted and deny sets from RFC 0006 section 7.
//!
//! RFC 0006 is a survey RFC: it records one disposition per surveyed candidate,
//! and the accepted set it leaves behind is what the child RFCs partition. This
//! module is the entry point that turns the document into that set. It holds the
//! derived result and its orchestration; the reading lives in [`section7`],
//! [`section8`], and [`totals`], and the guards in [`assertions`].
//!
//! The accepted set is 60 helpers: 57 new — 55 accept rows plus the two rename
//! exceptions that arrive from reject rows — and the 3 existing helpers gaining
//! a behaviour-preserving option. The deny set is the complement of the accepted
//! set within the rejected and deferred names: 71 names no child RFC may
//! register.
//!
//! Three quantities in that derivation are transcriptions rather than reads, all
//! of them in [`inventory`] and [`totals`]: the rename exceptions section 7.8
//! states in a sentence, the optioned helpers table 11 counts without naming,
//! and section 6.1's purity aggregate, which is spelled in number words. Each is
//! witnessed against the document, so a prose edit fails loudly instead of
//! drifting.
//!
//! A fourth quantity is deliberately **not** derived: table 11's split of the 50
//! reject rows into 22 already-provides, 10 redundant-alias, and 18
//! on-principle. RFC 0006 states no rule assigning a row to one of those
//! classes, and no rule fitted to the tables recovers the split. A three-way
//! rejection type would therefore be undecidable at runtime, which is why
//! `Disposition` has one `Reject` variant. See decision `D10` in the `ExecPlan`.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};

use super::{
    Namespace, RFC_0006, Repo, Row, Section,
    assertions::{check_against_document, check_denied},
    section7::{apply_optioned, apply_renames, read_section_7},
    section8::check_section_8,
    totals::{PROPOSED_HELPERS, purity_aggregate, table_11},
};

/// Everything the coverage checks need from RFC 0006.
pub(super) struct Survey {
    /// Accepted helpers by registered name, with the row that established them.
    pub(super) accepted: BTreeMap<String, Row>,
    /// The 57 new helpers, in document order.
    pub(super) new_rows: Vec<Row>,
    /// The 3 optioned helper names, in the order `inventory::OPTIONED` lists
    /// them.
    pub(super) optioned: Vec<String>,
    /// Names no child RFC may register.
    pub(super) denied: BTreeSet<String>,
    /// Accepted helper names per section 8 subsection.
    pub(super) sections: BTreeMap<String, BTreeSet<String>>,
    /// Accept rows parsed from section 7.
    pub(super) accept_rows: usize,
    /// Defer rows parsed from section 7.
    pub(super) defer_rows: usize,
    /// Reject rows parsed from section 7.
    pub(super) reject_rows: usize,
    /// Table 11's three reject-class counts, in table order.
    pub(super) reject_classes: [usize; 3],
    /// Table 11's new-filter count.
    pub(super) new_filters: usize,
    /// Table 11's new-test count.
    pub(super) new_tests: usize,
    /// Table 11's count of existing helpers gaining an option.
    pub(super) optioned_total: usize,
    /// Section 6.1's purity aggregate as (pure, filesystem, environment).
    pub(super) purity: (usize, usize, usize),
    /// Section 6.1's proposed-helper count.
    pub(super) proposed: usize,
}

impl Survey {
    /// Accepted helper names by namespace, over the new helpers only.
    pub(super) fn new_by_namespace(&self, namespace: Namespace) -> usize {
        self.new_rows
            .iter()
            .filter(|row| row.namespace == namespace)
            .count()
    }
}

/// Derive the survey and assert it against the document it came from.
///
/// The assertions are part of the derivation rather than a separate check
/// because every one of them is a cross-check between two places RFC 0006 states
/// the same thing: the row counts against table 11, the class counts against the
/// reject-row count, the new-filter and new-test counts against the tables, and
/// the deny set against the non-vacuity witnesses. A parse that silently returns
/// nothing must fail here.
pub(super) fn derive_and_check(repo: &Repo) -> Result<Survey> {
    let survey = derive(repo)?;
    check_against_document(&survey)?;
    check_denied(&survey)?;
    Ok(survey)
}

/// Read RFC 0006 and derive the sets and totals the coverage checks assert.
pub(super) fn derive(repo: &Repo) -> Result<Survey> {
    let text = repo.read(RFC_0006)?;
    let document = Section::whole(&text);
    let section_7 = document
        .subsection("## 7. Candidate matrix")
        .context("RFC 0006 has no section 7")?;

    let mut read = read_section_7(&section_7)?;
    apply_renames(&mut read)?;
    let optioned = apply_optioned(&mut read)?;
    check_section_8(&document, &read.accepted, &read.sections_of)?;

    // Deny set: the complement of the accepted set within the rejected and
    // deferred names. See decision `D10` in the ExecPlan.
    let mut deny_candidates = read.rejected.clone();
    deny_candidates.extend(read.deferred.iter().cloned());
    let accepted_names: BTreeSet<String> = read.accepted.keys().cloned().collect();
    let denied: BTreeSet<String> = deny_candidates
        .difference(&accepted_names)
        .cloned()
        .collect();

    let totals = table_11(&section_7)?;
    let purity = purity_aggregate(&document)?;

    let mut sections: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (name, section) in &read.sections_of {
        sections
            .entry(section.clone())
            .or_default()
            .insert(name.clone());
    }

    Ok(Survey {
        accepted: read.accepted,
        new_rows: read.new_rows,
        optioned,
        denied,
        sections,
        accept_rows: read.accept_rows,
        defer_rows: read.defer_rows,
        reject_rows: read.reject_rows,
        reject_classes: totals.reject_classes,
        new_filters: totals.new_filters,
        new_tests: totals.new_tests,
        optioned_total: totals.optioned,
        purity,
        proposed: PROPOSED_HELPERS,
    })
}
