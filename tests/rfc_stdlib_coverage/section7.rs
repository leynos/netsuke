//! Reading RFC 0006 section 7's disposition tables.
//!
//! The survey records one disposition per surveyed candidate across seven
//! tables. This module reads those tables, expands the alias groups the tables
//! collapse into single rows, and applies the two prose exceptions: section
//! 7.8's rename list, and the optioned helpers table 11 counts but never names.
//!
//! What it returns is [`Read`], not the accepted set. The caller still has to
//! take the complement of the rejected and deferred names against it, since the
//! deny set is defined by that complement rather than by any row.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::{
    Disposition, Namespace, RFC_0006, Row, Section, backticked,
    inventory::{CANDIDATE_TABLES, CandidateTable, OPTIONED, RENAMES},
    names_in,
};

/// What reading section 7 yields before the two prose exceptions are applied.
pub(super) struct Read {
    /// Accepted helpers by registered name.
    pub(super) accepted: BTreeMap<String, Row>,
    /// The new helpers, in document order.
    pub(super) new_rows: Vec<Row>,
    /// The section 8 subsection each accepted name is specified in.
    pub(super) sections_of: BTreeMap<String, String>,
    /// The section 8 subsection cited by each row of *any* disposition.
    ///
    /// Kept apart from `sections_of` because the optioned helpers are named by
    /// reject rows, whose names must not enter the section-8 membership the
    /// coverage map expands. Only [`apply_optioned`] reads this map.
    pub(super) cited: BTreeMap<String, String>,
    /// Every name that appears in a section 7 name cell.
    pub(super) surveyed_names: BTreeSet<String>,
    /// The namespace each surveyed name's own table places it in.
    ///
    /// Recorded rather than assumed, for the reason [`Optioned`]'s namespace
    /// field gives: the coverage check compares a child registry's namespace
    /// column against the value derived here, so a hardcoded default makes a
    /// *correct* child RFC fail the moment a renamed helper is not a filter.
    pub(super) namespaces: BTreeMap<String, Namespace>,
    /// Names section 9 defers.
    pub(super) deferred: BTreeSet<String>,
    /// Names section 10 rejects.
    pub(super) rejected: BTreeSet<String>,
    /// Accept rows read.
    pub(super) accept_rows: usize,
    /// Defer rows read.
    pub(super) defer_rows: usize,
    /// Reject rows read.
    pub(super) reject_rows: usize,
}

/// Read the seven disposition tables of section 7.
pub(super) fn read_section_7(section_7: &Section<'_>) -> Result<Read> {
    let mut read = Read {
        accepted: BTreeMap::new(),
        new_rows: Vec::new(),
        sections_of: BTreeMap::new(),
        cited: BTreeMap::new(),
        surveyed_names: BTreeSet::new(),
        namespaces: BTreeMap::new(),
        deferred: BTreeSet::new(),
        rejected: BTreeSet::new(),
        accept_rows: 0,
        defer_rows: 0,
        reject_rows: 0,
    };

    for (heading, rows) in section_7.tables() {
        let Some(table) = CANDIDATE_TABLES
            .iter()
            .find(|table| table.heading == heading)
        else {
            continue;
        };
        for row in &rows {
            let names = names_in(row.cell(0, "name")?);
            read.surveyed_names.extend(names.iter().cloned());
            record_namespaces(&mut read, &names, table.namespace)?;
            let disposition = row.cell(table.disposition_column, "disposition")?.trim();
            let resolution = row.cell(table.disposition_column + 1, "resolution")?;
            record_cited(&mut read, resolution, &names);
            match classify(disposition) {
                Some(Disposition::Accept) => {
                    read.accept_rows += 1;
                    let entry = Entry {
                        table,
                        names: &names,
                        disposition,
                        resolution,
                        line: row.line,
                    };
                    record_accept(&mut read, &entry)?;
                }
                Some(Disposition::Defer) => {
                    read.defer_rows += 1;
                    read.deferred.extend(names);
                }
                Some(Disposition::Reject) => {
                    read.reject_rows += 1;
                    read.rejected.extend(names);
                }
                None => {
                    return Err(anyhow::anyhow!(
                        "unknown disposition {disposition:?} at {RFC_0006}:{}",
                        row.line
                    ));
                }
            }
        }
    }
    Ok(read)
}

/// Record the namespace each name's table places it in.
///
/// A name may be surveyed by more than one table — `win_splitdrive` in the
/// filters table is the shape the rename exceptions turn on — and the tables
/// disagree about the namespace when a row is listed in the wrong one. Two
/// tables agreeing is not an error; two disagreeing is, because the derived
/// namespace would then depend on table order.
fn record_namespaces(read: &mut Read, names: &[String], namespace: Namespace) -> Result<()> {
    for name in names {
        let displaced = read.namespaces.insert(name.clone(), namespace);
        ensure!(
            displaced.is_none_or(|earlier| earlier == namespace),
            "surveyed name {name} is listed as both {} and {}, so section 7 places it in two \
             namespaces",
            displaced.map_or("no namespace", Namespace::label),
            namespace.label()
        );
    }
    Ok(())
}

/// Record the section 8 subsection a section 7 row cites, if it cites one.
///
/// Every disposition is recorded, not only acceptance. The three `Option added`
/// helpers are named by *reject* rows, which defer to section 8 for their
/// contract, and their names must not enter `sections_of`: that map is the
/// accepted set's section 8 membership, which the coverage map's `Owns` clauses
/// expand. Keeping the citation in a side map is what lets those three resolve
/// a section without joining the accepted set.
fn record_cited(read: &mut Read, resolution: &str, names: &[String]) {
    let Some(section) = section_ref(resolution) else {
        return;
    };
    for name in names {
        read.cited.insert(name.clone(), section.clone());
    }
}

/// One accept row of section 7, split into the columns the parse needs.
struct Entry<'a> {
    /// The table the row belongs to.
    table: &'a CandidateTable,
    /// The row's name cell, expanded into individual names.
    names: &'a [String],
    /// The row's disposition cell, trimmed.
    disposition: &'a str,
    /// The row's resolution cell.
    resolution: &'a str,
    /// One-indexed line the row was read from.
    line: usize,
}

/// Record one accept row's registered names.
///
/// The entry's `names` holds the row's surveyed spellings, which is normally
/// what is registered; its `disposition` overrides that for the one row that
/// reads "Accept as `text_hash`".
fn record_accept(read: &mut Read, entry: &Entry<'_>) -> Result<()> {
    let section = section_ref(entry.resolution).with_context(|| {
        format!(
            "accept row at {RFC_0006}:{} cites no section 8 subsection",
            entry.line
        )
    })?;
    for name in registered_names(entry.disposition, entry.names) {
        read.surveyed_names.insert(name.clone());
        read.sections_of.insert(name.clone(), section.clone());
        let helper = Row {
            name: name.clone(),
            namespace: entry.table.namespace,
        };
        if read.accepted.insert(name.clone(), helper.clone()).is_none() {
            read.new_rows.push(helper);
        }
    }
    Ok(())
}

/// Apply section 7.8's three rename exceptions.
///
/// `hash` is the odd one: its survey row is an accept row whose disposition cell
/// already names `text_hash`, so it is in; the other two are reject rows whose
/// *capability* is accepted under a new name.
pub(super) fn apply_renames(read: &mut Read) -> Result<()> {
    for rename in &RENAMES {
        ensure!(
            read.surveyed_names.contains(rename.surveyed),
            "rename exception {} has no section 7 row",
            rename.surveyed
        );
        let already = read.accepted.contains_key(rename.registered);
        ensure!(
            already == (rename.surveyed == "hash"),
            "rename exception {} -> {} is {} in the accepted set; only `hash` should already be",
            rename.surveyed,
            rename.registered,
            if already { "present" } else { "absent" }
        );
        if !already {
            let namespace = *read.namespaces.get(rename.surveyed).with_context(|| {
                format!(
                    "rename exception {} has no namespace, so section 7's tables place it nowhere",
                    rename.surveyed
                )
            })?;
            let entry = Row {
                name: rename.registered.to_owned(),
                namespace,
            };
            read.accepted
                .insert(rename.registered.to_owned(), entry.clone());
            read.new_rows.push(entry);
        }
        read.sections_of
            .insert(rename.registered.to_owned(), rename.section.to_owned());
    }
    Ok(())
}

/// Apply the three optioned helpers.
///
/// These are existing Netsuke helpers, so none contributes a new row: their
/// section 8 subsection is inherited from the section 7 row that cites them.
pub(super) fn apply_optioned(read: &mut Read) -> Result<Vec<String>> {
    let mut optioned = Vec::new();
    for option in &OPTIONED {
        ensure!(
            read.surveyed_names.contains(option.evidence_row),
            "the section 7 row citing optioned helper {} does not exist",
            option.name
        );
        let section = read
            .cited
            .get(option.evidence_row)
            .with_context(|| {
                format!(
                    "the section 7 row naming optioned helper {} cites no section 8 subsection",
                    option.name
                )
            })?
            .clone();
        read.sections_of.insert(option.name.to_owned(), section);
        // A failing insert rather than an overwrite: an optioned helper is named
        // by a *reject* row in section 7, so it never reaches `accepted` through
        // `record_accept`, and its namespace cannot come from the document. If
        // one is already present, section 7 also accepted it under the same
        // spelling, and the two records disagree about its namespace.
        let previous = read.accepted.insert(
            option.name.to_owned(),
            Row {
                name: option.name.to_owned(),
                namespace: option.namespace,
            },
        );
        ensure!(
            previous.is_none(),
            "optioned helper {} is already recorded as accepted, so section 7 both accepts it \
             and rejects it in favour of an option",
            option.name
        );
        optioned.push(option.name.to_owned());
    }
    Ok(optioned)
}

/// Classify a disposition cell.
pub(super) fn classify(disposition: &str) -> Option<Disposition> {
    let lower = disposition.to_ascii_lowercase();
    if lower == "accept" || lower.starts_with("accept as") {
        Some(Disposition::Accept)
    } else if lower == "defer" {
        Some(Disposition::Defer)
    } else if lower == "reject" || lower.starts_with("reject as") {
        Some(Disposition::Reject)
    } else {
        None
    }
}

/// The registered Netsuke names an accept row contributes.
///
/// Normally the row's own name. For the one row whose disposition cell reads
/// "Accept as `text_hash`", the cell names the spelling that is actually
/// registered, and the surveyed spelling is an existing helper left unchanged.
pub(super) fn registered_names(disposition: &str, names: &[String]) -> Vec<String> {
    if disposition.to_ascii_lowercase().starts_with("accept as") {
        let registered = backticked(disposition);
        if !registered.is_empty() {
            return registered;
        }
    }
    names.to_vec()
}

/// The first `§N.N` reference in `text`, normalized to `N.N`.
///
/// A sentence-final citation reads `§8.9.`, and the period ends the sentence
/// rather than extending the number: the digits-and-dots scan cannot tell the
/// two apart, so trailing dots are trimmed before the result is checked for
/// emptiness. `§.` therefore yields `None`, as does a citation that is absent.
///
/// Returns `None` when the cell cites no section, which for an accept row is an
/// error: every accepted helper must be specified somewhere in section 8.
pub(super) fn section_ref(text: &str) -> Option<String> {
    let rest = text.split_once('§')?.1;
    let end = rest
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .unwrap_or(rest.len());
    let found = rest.get(..end)?.trim_end_matches('.');
    if found.is_empty() {
        return None;
    }
    Some(found.to_owned())
}
