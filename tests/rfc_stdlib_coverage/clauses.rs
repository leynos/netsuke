//! The cross-cutting contract clauses and their discharge in child RFCs.
//!
//! RFC 0006 section 6 is normative and says so: "A child RFC that does not
//! satisfy every clause below for every helper it adds is not complete." The
//! clause list is therefore derived from the document rather than transcribed —
//! each `### 6.N.` subsection is a clause — and each child RFC must discharge
//! every one of them.
//!
//! The discharge is recorded in a child's section 5 as a two-column table,
//! `Clause | Discharge`, whose clause column holds the clause id in backticks.
//! The set of ids must equal section 6's subsection ids exactly. That is the
//! smallest contract that makes "discharges every clause" checkable without
//! pretending a test can read prose, and it is deliberately not a subsection
//! apiece: the eleven clause subsections record the group's own contract, and
//! the table records how each clause is met.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use super::{RFC_0006, Repo, Section, strip_backticks};

/// The heading of a child RFC's clause-discharge subsection.
///
/// Unnumbered on purpose. The table resolves all eleven clause subsections, and
/// numbering it `5.6` collided with the clause's own subsection title, "Type and
/// error contract" — the same title RFC 0006 clause 6.6 carries — leaving a
/// reader to reconcile a subsection titled one way with a table row reading
/// `6.6`. It closes section 5 as `### Clause discharge`.
const CLAUSES_HEADING: &str = "### Clause discharge";

/// The same heading as `tables()` reports it, without its `###`.
///
/// The parser resolves the table by the heading directly above it rather than
/// taking the subsection's first table, so a child that places a table under
/// one of the eleven clause subsections cannot have it mistaken for the
/// discharge table.
const TABLE_HEADING: &str = "Clause discharge";

/// RFC 0006 section 6's clause ids, in document order.
pub(super) fn clause_ids(repo: &Repo) -> Result<Vec<String>> {
    let text = repo.read(RFC_0006)?;
    let document = Section::whole(&text);
    let section = document
        .subsection("## 6. Cross-cutting contract")
        .context("RFC 0006 has no section 6")?;
    let mut ids = Vec::new();
    for line in &section.lines {
        let Some(heading) = line.strip_prefix("### ") else {
            continue;
        };
        let id = heading.split('.').take(2).collect::<Vec<_>>().join(".");
        if id.starts_with("6.") && !ids.contains(&id) {
            ids.push(id);
        }
    }
    ensure!(
        !ids.is_empty(),
        "RFC 0006 section 6 has no numbered subsections, so it states no clauses"
    );
    Ok(ids)
}

/// The clause ids a child RFC discharges.
pub(super) fn discharged(repo: &Repo, file: &str) -> Result<BTreeSet<String>> {
    let text = repo.read(file)?;
    let document = Section::whole(&text);
    let section = document
        .subsection(CLAUSES_HEADING)
        .with_context(|| format!("{file} has no clause-discharge table at {CLAUSES_HEADING}"))?;
    // Matched on the heading text rather than by taking the first table found:
    // the subsection is located by its own heading, and pairing the table with
    // the heading directly above it is what distinguishes it from any table a
    // child places under one of the eleven clause subsections above.
    let Some((_, rows)) = section
        .tables()
        .into_iter()
        .find(|(heading, rows)| !rows.is_empty() && heading == TABLE_HEADING)
    else {
        return Err(anyhow::anyhow!(
            "the clause-discharge subsection of {file} contains no table"
        ));
    };
    let mut ids = BTreeSet::new();
    for row in &rows {
        let cell = row.cell(0, "clause")?;
        let id = strip_backticks(cell).trim().to_owned();
        ensure!(
            id.starts_with("6."),
            "{file}:{} names clause {id:?}, which is not a section 6 clause",
            row.line
        );
        ensure!(
            !strip_backticks(row.cell(1, "discharge")?).trim().is_empty(),
            "{file}:{} discharges {id} with an empty cell",
            row.line
        );
        ids.insert(id);
    }
    Ok(ids)
}
