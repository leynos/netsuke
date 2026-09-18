//! The cross-cutting contract clauses and their discharge in child RFCs.
//!
//! RFC 0006 section 6 is normative and says so: "A child issue that does not
//! satisfy every clause below for every helper it adds is not complete." The
//! clause list is therefore derived from the document rather than transcribed —
//! each `### 6.N.` subsection is a clause — and each child RFC must discharge
//! every one of them.
//!
//! The discharge is recorded in a child's section 5.6 as a two-column table,
//! `Clause | Discharge`, whose clause column holds the clause id in backticks.
//! The set of ids must equal section 6's subsection ids exactly. That is the
//! smallest contract that makes "discharges every clause" checkable without
//! pretending a test can read prose.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use super::{RFC_0006, Repo, Section, strip_backticks};

/// The heading of a child RFC's clause-discharge table.
const CLAUSES_HEADING: &str = "### 5.6. Clause discharge";

/// RFC 0006 section 6's clause ids, in document order.
pub(super) fn clause_ids(repo: &Repo) -> Result<Vec<String>> {
    let text = repo.read(RFC_0006)?;
    let document = Section::whole(&text, RFC_0006);
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
    let document = Section::whole(&text, file);
    let section = document
        .subsection(CLAUSES_HEADING)
        .with_context(|| format!("{file} has no clause-discharge table at {CLAUSES_HEADING}"))?;
    let Some((_, rows)) = section
        .tables()
        .into_iter()
        .find(|(heading, rows)| heading.starts_with("5.6.") && !rows.is_empty())
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
