//! Parsers and checks backing the RFC 0006 child-RFC coverage contract.
//!
//! RFC 0006 is a survey RFC: it enumerates every ansible-core candidate it
//! considered and records a disposition for each. This module tree turns that
//! survey, the roadmap, and the child RFCs into a single machine-checkable
//! bijection, so a helper cannot be dropped, double-assigned, or reintroduced
//! under a rejected spelling without a test naming the file and line.
//!
//! Everything here is derived. The accepted set, the deny set, the expected
//! owners, and the clause list are all read out of tracked Markdown. What cannot
//! be derived is transcribed, and each transcription is witnessed against the
//! document it came from: the section 7 inventories and section 6.1's purity
//! aggregate in `survey`, and the ownership grammar of the coverage map's `Owns`
//! column in `map`. RFC 0006 states the former in prose rather than in a table,
//! and the latter is a mini-language no row spells out.
//!
//! The parser contract is deliberately narrow: it reads Markdown table rows
//! inside a named section and never headings. Heading anchoring was rejected
//! because three headings under RFC 0006 section 8 are prose rather than
//! helpers, three name two helpers each, and `basename` and `dirname` have no
//! heading at all. Names are matched by whole-token equality, never substring,
//! because the vocabulary contains `abs` against `is_abs`, `quote` against
//! `shell_quote`, `hash` against `text_hash`, and `subset` against
//! `issubset`.
//!
//! Fenced code blocks are opaque to every scan. A child RFC's section 5 carries
//! example blocks — Jinja snippets, diagnostic text, shell — and a `#`, a `|`,
//! or a backticked name inside one is example content rather than structure.

mod assertions;
mod clauses;
mod document;
mod inventory;
mod links;
mod map;
mod markdown;
mod partition;
mod progress;
mod registries;
mod roadmap;
mod section7;
mod section8;
mod survey;
mod totals;

pub use partition::{
    every_accepted_helper_has_exactly_one_owner, no_forbidden_helper_is_registered,
};
pub use progress::{
    coverage_map_status_is_reported, every_capability_has_a_roadmap_task,
    every_child_discharges_every_clause, inter_document_links_resolve,
    totals_and_purity_aggregate_agree,
};
// Private, but reachable as `super::…` from every child module, which is how
// `section8` gets at `Fences` and `heading_depth`.
use markdown::{Fences, heading_depth, is_separator, row_cells, table_heading};
// Same arrangement, and for the same reason: the document layer moved to its
// own module to stay under the 400-line cap, so this import is what keeps every
// `super::Section` path in the module tree resolving as it did before the split.
use document::{RawRow, Section};

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Repository-relative path of RFC 0006, the survey RFC.
pub(super) const RFC_0006: &str = "docs/rfcs/0006-ansible-inspired-template-standard-library.md";

/// Repository-relative path of the delivery roadmap.
pub(super) const ROADMAP: &str = "docs/roadmap.md";

/// Repository-relative directory holding the RFC corpus.
pub(super) const RFC_DIR: &str = "docs/rfcs";

/// Jinja namespace a helper occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Namespace {
    /// Invoked as a filter on a piped value.
    Filter,
    /// Invoked as a test after `is`.
    Test,
    /// Invoked as a bare function call.
    Function,
}

impl Namespace {
    /// The lowercase label RFC 0006's tables spell the namespace with.
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Filter => "filter",
            Self::Test => "test",
            Self::Function => "function",
        }
    }
}

/// Whether a registry row introduces a helper or adds an option to one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Registration {
    /// One of the 57 new helpers.
    New,
    /// One of the 3 existing helpers gaining an option.
    OptionAdded,
}

impl Registration {
    /// The lowercase label a child RFC's registry spells the kind with.
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::OptionAdded => "option added",
        }
    }
}

/// A surveyed name's disposition, derived from RFC 0006 section 7.
///
/// Rejection is one variant, not three. RFC 0006 table 11 splits the 50 reject
/// rows into 22 already-provides, 10 redundant-alias, and 18 on-principle, but
/// the document states no rule assigning a row to a class and no rule fitted to
/// the tables recovers the split, so a three-way rejection type here would be
/// undecidable at runtime. Under decision `D10` the deny set is the complement
/// of the accepted set and needs no classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Disposition {
    /// Accepted, with the owning section 8 subsection.
    Accept,
    /// Deferred by section 9.
    Defer,
    /// Rejected by section 10, for any of the three reasons table 11 counts.
    Reject,
}

/// One accepted helper, as the document that established it records it.
///
/// The row deliberately carries no source location: every parser that builds one
/// already reports `file:line` in the message that rejects a bad row, and no
/// later check has a use for the provenance.
#[derive(Debug, Clone)]
pub(super) struct Row {
    /// Registered helper name, backticks stripped.
    name: String,
    /// Namespace the helper occupies.
    namespace: Namespace,
}

/// A capability-scoped view of the repository working tree.
///
/// The handle is rooted at the package directory Cargo built this test from,
/// so every path in this module is repository-relative and independent of the
/// working directory the test happens to be run from.
pub(super) struct Repo {
    /// Directory handle rooted at the package root.
    dir: Dir,
}

impl Repo {
    /// Open the package root named by Cargo at build time.
    pub(super) fn open() -> Result<Self> {
        let root = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
        let dir = Dir::open_ambient_dir(root, ambient_authority())
            .with_context(|| format!("open repository root {root}"))?;
        Ok(Self { dir })
    }

    /// Read a repository-relative file as UTF-8.
    fn read(&self, path: &str) -> Result<String> {
        self.dir
            .read_to_string(path)
            .with_context(|| format!("read {path}"))
    }

    /// Whether a repository-relative path exists.
    fn exists(&self, path: &str) -> Result<bool> {
        self.dir
            .try_exists(path)
            .with_context(|| format!("test existence of {path}"))
    }

    /// Repository-relative paths of the `.md` files directly under `dir`.
    ///
    /// Entries are sorted, so a parse over the corpus is deterministic and a
    /// failure message names a stable file.
    fn markdown_files(&self, dir: &str) -> Result<Vec<String>> {
        let mut found = Vec::new();
        let entries = self
            .dir
            .read_dir(dir)
            .with_context(|| format!("list {dir}"))?;
        for candidate in entries {
            let entry = candidate.with_context(|| format!("read a {dir} directory entry"))?;
            let name = entry
                .file_name()
                .with_context(|| format!("read a {dir} entry name"))?;
            let path = Utf8Path::new(&name);
            if path.extension() == Some("md") {
                found.push(format!("{dir}/{name}"));
            }
        }
        found.sort_unstable();
        Ok(found)
    }
}

/// Strip one surrounding pair of backticks and any surrounding whitespace.
///
/// A cell may hold several backticked names separated by ` / `; callers that
/// expect that split on [`NAME_SEPARATOR`] first.
pub(super) fn strip_backticks(cell: &str) -> &str {
    let trimmed = cell.trim();
    trimmed
        .strip_prefix('`')
        .and_then(|rest| rest.strip_suffix('`'))
        .unwrap_or(trimmed)
}

/// The separator RFC 0006 section 7.8 uses between alias-group names.
pub(super) const NAME_SEPARATOR: &str = " / ";

/// Expand a name cell into its individual backtick-stripped names.
///
/// Section 7.8 records that rows cover alias groups as single entries, so
/// `` `d` / `default` `` contributes two names.
pub(super) fn names_in(cell: &str) -> Vec<String> {
    if cell.trim() == "—" || cell.trim() == "-" {
        return Vec::new();
    }
    cell.split(NAME_SEPARATOR)
        .map(|name| strip_backticks(name).to_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

/// Every backticked span in `text`, in order.
pub(super) fn backticked(text: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .map(ToOwned::to_owned)
        .collect()
}

/// A sorted, de-duplicated view of `names`, for diffing in assertions.
pub(super) fn name_set<'a>(names: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
    names.into_iter().map(ToOwned::to_owned).collect()
}

/// Context every check needs, derived once per call.
///
/// Owned here rather than by either check module because both read it: the
/// ownership partition and the progress totals derive from the same four
/// documents, and a single parse bug must surface in whichever check depends on
/// the part it corrupts.
pub(in crate::rfc_stdlib_coverage) struct World {
    /// The parse of RFC 0006's dispositions and totals.
    pub(in crate::rfc_stdlib_coverage) survey: survey::Survey,
    /// The parse of RFC 0006's coverage map.
    pub(in crate::rfc_stdlib_coverage) map: map::Map,
    /// Every existing child RFC's registry.
    pub(in crate::rfc_stdlib_coverage) registries: Vec<registries::Registry>,
    /// The roadmap's capability steps.
    pub(in crate::rfc_stdlib_coverage) roadmap: roadmap::Steps,
    /// The failure path each child RFC's number resolves to, when written.
    pub(in crate::rfc_stdlib_coverage) child_paths: std::collections::BTreeMap<String, String>,
}

impl World {
    /// Derive everything from the working tree.
    pub(in crate::rfc_stdlib_coverage) fn load(repo: &Repo) -> Result<Self> {
        let survey = survey::derive_and_check(repo)?;
        let sections = survey
            .sections
            .iter()
            .map(|(section, names)| (section.clone(), names.iter().cloned().collect()))
            .collect();
        let parsed = map::parse(repo, &sections)?;
        let reserved: Vec<String> = parsed.rows.iter().map(|row| row.number.clone()).collect();
        let child_paths = parsed
            .rows
            .iter()
            .filter_map(|row| {
                row.written
                    .as_ref()
                    .map(|target| (row.number.clone(), target.clone()))
            })
            .collect();
        Ok(Self {
            survey,
            map: parsed,
            registries: registries::parse_all(repo, &reserved)?,
            roadmap: roadmap::parse(repo)?,
            child_paths,
        })
    }
}
