//! Reading the five-column registry every child RFC carries.
//!
//! Each child RFC's section 5.1 is a table with one row per helper it owns:
//! Helper, Namespace, Registration, Purity class, Manifest query. That table is
//! the artefact RFC 0006 section 6.1 asks for — the per-helper record of the
//! contract that section 6.1, table 3, section 6.2, and section 6.9 impose —
//! and it is also the machine-readable anchor for the ownership bijection.
//!
//! Sections 5.2 to 5.5 of each child discharge the remaining contract clauses
//! per helper. This module reads only the registry; [`super::clauses`] reads the
//! rest.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use super::{Namespace, RawRow, Registration, Repo, Row, Section, strip_backticks};

/// The heading of a child RFC's registry table.
const REGISTRY_HEADING: &str = "### 5.1. Registry";

/// Purity classes as RFC 0006 table 2 names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Purity {
    /// Result depends only on the supplied value and arguments.
    Pure,
    /// Reads the wall clock.
    Clock,
    /// Reads process environment variables.
    Environment,
    /// Reads filesystem metadata or contents.
    Filesystem,
    /// Performs a network request.
    Network,
    /// Spawns a child process.
    Subprocess,
}

impl Purity {
    /// Parse a purity cell, which must spell the class as table 2 does.
    pub(super) fn parse(cell: &str) -> Option<Self> {
        match cell.trim().to_ascii_lowercase().as_str() {
            "pure" => Some(Self::Pure),
            "clock-observing" => Some(Self::Clock),
            "environment-observing" => Some(Self::Environment),
            "filesystem-observing" => Some(Self::Filesystem),
            "network-observing" => Some(Self::Network),
            "subprocess-observing" => Some(Self::Subprocess),
            _ => None,
        }
    }

    /// Whether RFC 0006 table 2 admits this class to manifest queries.
    const fn manifest_query(self) -> bool {
        matches!(self, Self::Pure)
    }

    /// The lowercase label used in failure messages.
    const fn label(self) -> &'static str {
        match self {
            Self::Pure => "pure",
            Self::Clock => "clock-observing",
            Self::Environment => "environment-observing",
            Self::Filesystem => "filesystem-observing",
            Self::Network => "network-observing",
            Self::Subprocess => "subprocess-observing",
        }
    }
}

/// One row of a child RFC's registry.
pub(super) struct RegistryRow {
    /// The helper the row registers or extends.
    pub(super) helper: Row,
    /// Whether the row introduces a helper or adds an option to an existing one.
    pub(super) registration: super::Registration,
    /// The helper's purity class.
    pub(super) purity: Purity,
}

/// A child RFC's registry.
pub(super) struct Registry {
    /// Repository-relative path of the child RFC.
    pub(super) file: String,
    /// The RFC number the file reserves, as `N`.
    pub(super) number: String,
    /// Its rows, in document order.
    pub(super) rows: Vec<RegistryRow>,
}

impl Registry {
    /// The helper names the registry lists.
    pub(super) fn names(&self) -> BTreeSet<String> {
        self.rows
            .iter()
            .map(|row| row.helper.name.clone())
            .collect()
    }

    /// The registered names with the given purity class.
    pub(super) fn with_purity(&self, purity: Purity) -> usize {
        self.rows.iter().filter(|row| row.purity == purity).count()
    }

    /// The registered names with the given registration kind.
    pub(super) fn with_registration(&self, registration: super::Registration) -> usize {
        self.rows
            .iter()
            .filter(|row| row.registration == registration)
            .count()
    }
}

/// Read one child RFC's registry.
pub(super) fn parse(repo: &Repo, file: &str) -> Result<Registry> {
    let text = repo.read(file)?;
    let document = Section::whole(&text, file);
    let number = rfc_number(file).with_context(|| {
        format!("{file} is not named as an RFC; expected a leading four-digit number")
    })?;
    ensure!(
        number.as_str() >= "0013",
        "{file} reserves RFC {number}; child RFCs start at 0013"
    );
    let section = document
        .subsection(REGISTRY_HEADING)
        .with_context(|| format!("{file} has no registry table at {REGISTRY_HEADING}"))?;
    let Some((_, rows)) = section
        .tables()
        .into_iter()
        .find(|(heading, rows)| heading.starts_with("5.1.") && !rows.is_empty())
    else {
        return Err(anyhow::anyhow!(
            "the registry subsection of {file} contains no table"
        ));
    };

    let mut parsed = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for row in &rows {
        let parsed_row = parse_row(file, row, &mut seen)?;
        parsed.push(parsed_row);
    }
    Ok(Registry {
        file: file.to_owned(),
        number,
        rows: parsed,
    })
}

/// Read one registry row, rejecting a name the registry already lists.
fn parse_row(file: &str, row: &RawRow, seen: &mut BTreeSet<String>) -> Result<RegistryRow> {
    let name = strip_backticks(row.cell(0, "helper")?).to_owned();
    ensure!(
        !name.is_empty(),
        "{file}:{} has an empty helper cell",
        row.line
    );
    ensure!(
        seen.insert(name.clone()),
        "{file}:{} registers {name} twice",
        row.line
    );
    let namespace = parse_namespace(file, row, &name)?;
    let registration = parse_registration(file, row, &name)?;
    let purity_cell = row.cell(3, "purity class")?;
    let purity = Purity::parse(purity_cell).with_context(|| {
        format!(
            "{file}:{} gives {name} purity class {purity_cell:?}, which RFC 0006 table 2 does not \
             define",
            row.line
        )
    })?;
    check_manifest_query(file, row, &name, purity)?;
    Ok(RegistryRow {
        helper: Row { name, namespace },
        registration,
        purity,
    })
}

/// Read a row's namespace cell.
fn parse_namespace(file: &str, row: &RawRow, name: &str) -> Result<Namespace> {
    let cell = row.cell(1, "namespace")?.trim().to_ascii_lowercase();
    match cell.as_str() {
        "filter" => Ok(Namespace::Filter),
        "test" => Ok(Namespace::Test),
        "function" => Ok(Namespace::Function),
        other => Err(anyhow::anyhow!(
            "{file}:{} gives {name} namespace {other:?}; expected `filter`, `test`, or `function`",
            row.line
        )),
    }
}

/// Read a row's registration cell.
fn parse_registration(file: &str, row: &RawRow, name: &str) -> Result<Registration> {
    let cell = strip_backticks(row.cell(2, "registration")?)
        .trim()
        .to_ascii_lowercase();
    match cell.as_str() {
        "new" => Ok(Registration::New),
        "option added" => Ok(Registration::OptionAdded),
        other => Err(anyhow::anyhow!(
            "{file}:{} gives {name} registration {other:?}; expected `new` or `option added`",
            row.line
        )),
    }
}

/// Assert a row's manifest-query cell agrees with its purity class.
///
/// RFC 0006 table 2 admits only pure helpers to manifest queries, so the two
/// cells are not independent.
fn check_manifest_query(file: &str, row: &RawRow, name: &str, purity: Purity) -> Result<()> {
    let cell = row.cell(4, "manifest query")?.trim().to_ascii_lowercase();
    let declared = match cell.as_str() {
        "yes" => true,
        "no" => false,
        other => {
            return Err(anyhow::anyhow!(
                "{file}:{} gives {name} manifest query {other:?}; expected `yes` or `no`",
                row.line
            ));
        }
    };
    ensure!(
        declared == purity.manifest_query(),
        "{file}:{} registers {name} as {} with manifest query {cell}; RFC 0006 table 2 says {}",
        row.line,
        purity.label(),
        if purity.manifest_query() { "yes" } else { "no" }
    );
    Ok(())
}

/// The RFC number a child RFC's filename reserves.
pub(super) fn rfc_number(file: &str) -> Option<String> {
    let name = file.rsplit('/').next()?;
    let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
    (digits.len() == 4).then_some(digits)
}

/// Every child RFC that exists, sorted by number.
pub(super) fn parse_all(repo: &Repo) -> Result<Vec<Registry>> {
    let mut found = Vec::new();
    for file in repo.markdown_files(super::RFC_DIR)? {
        let Some(number) = rfc_number(&file) else {
            continue;
        };
        if number.as_str() >= "0013" && repo.exists(&file)? {
            found.push(parse(repo, &file)?);
        }
    }
    found.sort_by(|left, right| left.number.cmp(&right.number));
    Ok(found)
}
