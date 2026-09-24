//! Reading and checking RFC 0006's coverage map.
//!
//! The map is a table in RFC 0006 section 14.13 with one row per capability
//! group. It is simultaneously the artefact RFC 0006 section 6.1 asks for and
//! the anchor for the ownership bijection: each row names the section 8
//! subsections a child RFC owns, and the rows together must partition the
//! accepted set exactly.
//!
//! The `Owns` grammar is deliberately tiny, because a richer one would be a
//! language nobody reviews. Clause forms, separated by semicolons:
//!
//! - `` `8.6` `` — every helper section 8.6 specifies;
//! - `` `8.6` except `expandvars` `` — all of them but the named one;
//! - `` `8.7` only `abs` `` — the named one alone.
//!
//! It is ASCII on purpose. The document's own section references use `§`, but no
//! Rust source in this repository does, and a map whose grammar is borrowed from
//! prose punctuation is harder to review than one that spells out `only` and
//! `except`.

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use super::{RFC_0006, RawRow, Repo, Section, backticked, registries};

/// The heading of the coverage map's subsection in RFC 0006 section 14.
const MAP_HEADING: &str = "### 14.13. Coverage map";

/// One row of the coverage map.
pub(super) struct MapRow {
    /// The RFC number reserved for this child, without the `RFC` prefix.
    pub(super) number: String,
    /// Repository-relative path of the child RFC, when it has been written.
    pub(super) written: Option<String>,
    /// The child RFC's title.
    #[expect(dead_code, reason = "read when a child's title is cross-checked")]
    pub(super) title: String,
    /// The helpers this child owns, resolved from the `Owns` clauses.
    pub(super) owns: Vec<String>,
    /// The existing helpers this child adds an option to.
    pub(super) optioned: Vec<String>,
    /// The roadmap step that delivers the group.
    pub(super) step: String,
    /// Whether the child RFC has been written.
    pub(super) is_written: bool,
}

impl MapRow {
    /// Every helper this row claims, owned or optioned.
    pub(super) fn claims(&self) -> Vec<String> {
        let mut all = self.owns.clone();
        all.extend(self.optioned.iter().cloned());
        all
    }
}

/// The parsed coverage map.
pub(super) struct Map {
    /// Its rows, in document order.
    pub(super) rows: Vec<MapRow>,
}

impl Map {
    /// The number of rows whose child RFC is still unwritten.
    pub(super) fn unwritten(&self) -> usize {
        self.rows.iter().filter(|row| !row.is_written).count()
    }

    /// Map every claimed helper name to the child that claims it.
    ///
    /// Fails when two rows claim the same helper, naming both, which is the
    /// two-owner control the ownership obligation needs.
    pub(super) fn ownership(&self) -> Result<BTreeMap<String, String>> {
        let mut owners: BTreeMap<String, String> = BTreeMap::new();
        for row in &self.rows {
            claim(row, &mut owners)?;
        }
        Ok(owners)
    }
}

/// Record one row's claims, failing when another row already claimed a name.
fn claim(row: &MapRow, owners: &mut BTreeMap<String, String>) -> Result<()> {
    for name in row.claims() {
        if let Some(previous) = owners.get(&name)
            && previous != &row.number
        {
            return Err(anyhow::anyhow!(
                "helper {name} is claimed by both RFC {previous} and RFC {}",
                row.number
            ));
        }
        owners.insert(name, row.number.clone());
    }
    Ok(())
}

/// Read the coverage map from RFC 0006.
pub(super) fn parse(repo: &Repo, sections: &BTreeMap<String, Vec<String>>) -> Result<Map> {
    let text = repo.read(RFC_0006)?;
    let document = Section::whole(&text);
    let map = document.subsection(MAP_HEADING).with_context(|| {
        format!(
            "RFC 0006 section 14 contains no coverage map table at {MAP_HEADING}; \
             expected 8 rows"
        )
    })?;

    let rows = map
        .tables()
        .into_iter()
        .find(|(heading, rows)| heading_is_map(heading) && !rows.is_empty())
        .map(|(_, rows)| rows)
        .context("the coverage map subsection contains no table")?;

    ensure!(
        rows.len() == 8,
        "the coverage map has {} rows; expected 8, one per capability group",
        rows.len()
    );

    let mut parsed = Vec::new();
    for row in &rows {
        parsed.push(parse_row(row, sections)?);
    }
    Ok(Map { rows: parsed })
}

/// Read one coverage-map row.
fn parse_row(row: &RawRow, sections: &BTreeMap<String, Vec<String>>) -> Result<MapRow> {
    let child = row.cell(0, "child RFC")?;
    let (number, written) = child_number(child).with_context(|| {
        format!(
            "coverage map row at {RFC_0006}:{} has an unreadable child RFC cell: {child}",
            row.line
        )
    })?;
    let is_written = parse_status(row, &number, written.is_some())?;
    // A written row links to a file, and the file's name carries the RFC
    // number the row reserves. The two are written by hand and nothing else
    // compares them: without this, row `0013` could link to `0014-….md`, and
    // every check downstream would resolve the link, find the file, and pass
    // — while `registries::parse_all` would file that document's helpers
    // under RFC 0013's name. The link is the only place the mismatch is
    // visible before the child is parsed.
    check_link_names_the_row(row, &number, written.as_deref())?;
    Ok(MapRow {
        number,
        written,
        title: row.cell(1, "title")?.trim().to_owned(),
        owns: resolve_owns(row.cell(2, "owns")?, sections, row.line)?,
        optioned: backticked(row.cell(3, "optioned")?),
        step: row.cell(4, "roadmap step")?.trim().to_owned(),
        is_written,
    })
}

/// Read a row's status cell, checking it agrees with whether it links.
fn parse_status(row: &RawRow, number: &str, links: bool) -> Result<bool> {
    let status = row.cell(5, "status")?.trim().to_ascii_lowercase();
    let is_written = match status.as_str() {
        "written" => true,
        "unwritten" => false,
        other => {
            return Err(anyhow::anyhow!(
                "coverage map row for RFC {number} at {RFC_0006}:{} has status {other:?}; \
                 expected `written` or `unwritten`",
                row.line
            ));
        }
    };
    ensure!(
        is_written == links,
        "coverage map row for RFC {number} is marked {status} but its child RFC cell is {}",
        if links { "a link" } else { "not a link" }
    );
    Ok(is_written)
}

/// Assert a written row's link carries the RFC number the row reserves.
fn check_link_names_the_row(row: &RawRow, number: &str, link: Option<&str>) -> Result<()> {
    let Some(target) = link else {
        return Ok(());
    };
    let target_number = registries::rfc_number(target);
    ensure!(
        target_number.as_deref() == Some(number),
        "coverage map row for RFC {number} at {RFC_0006}:{} links to {target}, whose \
         number is {}",
        row.line,
        target_number.as_deref().unwrap_or("not an RFC number")
    );
    Ok(())
}

/// Whether a table heading belongs to the coverage map.
pub(super) fn heading_is_map(heading: &str) -> bool {
    heading.starts_with("14.13.")
}

/// Read a child RFC cell, which is a link once written and a code span before.
///
/// Returns the row's RFC number and, when written, the link target. The link
/// text is the same four-digit number as the bare form and is unwrapped the same
/// way, so a written row is not merely a differently-shaped cell: both forms
/// have to yield a number the caller can compare against the map's own row.
///
/// The number is required to be exactly four ASCII digits. A cell reading `13`,
/// `0013b`, or `٠٠١٣` is not a reserved number, and accepting it would let a
/// link whose text disagrees with its target through the caller's comparison.
pub(super) fn child_number(cell: &str) -> Option<(String, Option<String>)> {
    let trimmed = cell.trim();
    let (text, target) = match trimmed.strip_prefix('[') {
        Some(rest) => {
            let (text, tail) = rest.split_once("](")?;
            let (target, _) = tail.split_once(')')?;
            (text.trim(), Some(target.trim()))
        }
        None => (trimmed.trim_matches('`').trim(), None),
    };
    let number = text.to_owned();
    is_rfc_number(&number).then_some((number, target.map(ToOwned::to_owned)))
}

/// Whether `text` is a four-digit RFC number.
fn is_rfc_number(text: &str) -> bool {
    text.len() == 4 && text.chars().all(|ch| ch.is_ascii_digit())
}

/// Resolve an `Owns` cell into the helper names it claims.
pub(super) fn resolve_owns(
    cell: &str,
    sections: &BTreeMap<String, Vec<String>>,
    line: usize,
) -> Result<Vec<String>> {
    let mut claimed = Vec::new();
    for clause in cell
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let tokens = backticked(clause);
        let section = tokens.first().with_context(|| {
            format!("`Owns` clause {clause:?} at {RFC_0006}:{line} names no section")
        })?;
        let members = sections.get(section).with_context(|| {
            format!(
                "`Owns` clause {clause:?} at {RFC_0006}:{line} names section {section}, which \
                     specifies no accepted helper"
            )
        })?;
        let lower = clause.to_ascii_lowercase();
        if lower.contains(" except ") {
            let excluded = tokens.get(1).with_context(|| {
                format!("`Owns` clause {clause:?} at {RFC_0006}:{line} excludes nothing")
            })?;
            ensure!(
                members.contains(excluded),
                "`Owns` clause {clause:?} at {RFC_0006}:{line} excludes {excluded}, which section \
                 {section} does not specify"
            );
            claimed.extend(members.iter().filter(|name| *name != excluded).cloned());
        } else if lower.contains(" only ") {
            let included = tokens.get(1).with_context(|| {
                format!("`Owns` clause {clause:?} at {RFC_0006}:{line} includes nothing")
            })?;
            ensure!(
                members.contains(included),
                "`Owns` clause {clause:?} at {RFC_0006}:{line} includes {included}, which section \
                 {section} does not specify"
            );
            claimed.push(included.clone());
        } else {
            ensure!(
                tokens.len() == 1,
                "`Owns` clause {clause:?} at {RFC_0006}:{line} names section {section} with extra \
                 backticked tokens; use `except` or `only`"
            );
            claimed.extend(members.iter().cloned());
        }
    }
    ensure!(
        !claimed.is_empty(),
        "`Owns` cell at {RFC_0006}:{line} claims no helper"
    );
    Ok(claimed)
}
