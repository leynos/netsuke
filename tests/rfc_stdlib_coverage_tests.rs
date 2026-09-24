//! Coverage contract for RFC 0006's split into focused child RFCs.
//!
//! RFC 0006 surveys 111 ansible-core candidates, accepts 60 helpers, defers 6,
//! and rejects 50. Roadmap 6.1.1 splits that accepted set into eight child RFCs
//! and requires that every accepted capability is covered exactly once and every
//! deferred or rejected candidate is covered not at all. Fifty-seven names are
//! not reliably partitioned by reading, so this binary derives the sets from
//! RFC 0006's own tables and asserts the partition.
//!
//! The helper sets are derived from tracked Markdown and checked against
//! tracked Markdown, so no count of names is carried here that could drift from
//! the documents. What the derivation rests on are anchors it cannot parse out
//! of prose — the seven candidate-table headings, the three renames, the three
//! optioned helpers, and section 6.1's proposed-helper count — and each of those
//! is transcribed and then witnessed against the document it came from rather
//! than trusted as written. The derivation lives in the
//! [`rfc_stdlib_coverage`] module tree, one module per source document, so a
//! failure names the obligation it discharges rather than a line in a helper.
//!
//! Failures name the file and line of the offending row, because the fix is
//! almost always an edit to a document rather than to this test.
//!
//! Each check is called through its module path rather than imported, so the
//! test function and the check it runs can share a name.

mod rfc_stdlib_coverage;

use anyhow::Result;

use rfc_stdlib_coverage::Repo;

/// Run `check` against a freshly opened view of the repository.
///
/// Each test opens its own handle so the tests can run in any order and in
/// parallel without sharing state.
fn run(check: impl FnOnce(&Repo) -> Result<()>) -> Result<()> {
    let repo = Repo::open()?;
    check(&repo)
}

#[test]
fn every_accepted_helper_has_exactly_one_owner() -> Result<()> {
    run(rfc_stdlib_coverage::every_accepted_helper_has_exactly_one_owner)
}

#[test]
fn no_forbidden_helper_is_registered() -> Result<()> {
    run(rfc_stdlib_coverage::no_forbidden_helper_is_registered)
}

#[test]
fn totals_and_purity_aggregate_agree() -> Result<()> {
    run(rfc_stdlib_coverage::totals_and_purity_aggregate_agree)
}

#[test]
fn coverage_map_status_is_reported() -> Result<()> {
    run(rfc_stdlib_coverage::coverage_map_status_is_reported)
}

#[test]
fn inter_document_links_resolve() -> Result<()> {
    run(rfc_stdlib_coverage::inter_document_links_resolve)
}

#[test]
fn every_capability_has_a_roadmap_task() -> Result<()> {
    run(rfc_stdlib_coverage::every_capability_has_a_roadmap_task)
}

#[test]
fn every_child_discharges_every_clause() -> Result<()> {
    run(rfc_stdlib_coverage::every_child_discharges_every_clause)
}
