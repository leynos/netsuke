//! Cross-checks between the derived sets and the words RFC 0006 states them in.
//!
//! These are part of the derivation rather than a separate check, because every
//! one of them compares two places the RFC states the same thing: the row counts
//! against table 11, the class counts against the reject-row count, the
//! new-filter and new-test counts against the tables, and the deny set against
//! the non-vacuity witnesses. A parse that silently returns nothing must fail
//! here.
//!
//! One comparison in this module is not of that shape. The deny set's size is
//! checked against the literal 71, because the complement rule that produces the
//! deny set has no second statement in the document to read a figure from: the
//! rule is this plan's decision `D10`, and 71 is what it yields over RFC 0006's
//! own reject and defer rows. That literal is the assertion. It is not a
//! transcription of a sentence somewhere that a rebase could leave behind, so
//! it is deliberately not moved into the table-11 reads above with the counts
//! that are.

use anyhow::{Result, ensure};

use super::{Namespace, name_set, survey::Survey};

/// Assert the derived totals agree with what RFC 0006 states.
pub(super) fn check_against_document(survey: &Survey) -> Result<()> {
    // The accept and defer counts are compared against table 11's own rows
    // rather than against transcribed literals, because the table is what the
    // comparison is between: a literal here would make this an assertion about
    // a constant, and would have to be edited by hand every time the survey
    // legitimately grew. Table 11 is expected to agree with section 7, and if
    // it stops agreeing that is the finding, not a reason to re-read the table.
    ensure!(
        survey.accept_rows == survey.stated_accept,
        "derived {} accept rows in RFC 0006 section 7; table 11 states {}",
        survey.accept_rows,
        survey.stated_accept
    );
    ensure!(
        survey.defer_rows == survey.stated_defer,
        "derived {} defer rows in RFC 0006 section 7; table 11 states {}",
        survey.defer_rows,
        survey.stated_defer
    );
    // The reject count is the one row count with no table 11 row of its own:
    // table 11 splits rejections across three classes, so the sum of those three
    // rows is what section 7's reject count has to equal. Both sides of the
    // comparison are read from the document, which is why this needs no
    // literal — and it is also why the message names both figures rather than
    // asserting the equality of one of them to a constant.
    let class_sum: usize = survey.reject_classes.iter().sum();
    ensure!(
        survey.reject_rows == class_sum,
        "derived {} reject rows in RFC 0006 section 7; table 11's classes {:?} sum to {class_sum}",
        survey.reject_rows,
        survey.reject_classes
    );
    ensure!(
        survey.new_by_namespace(Namespace::Filter) == survey.new_filters,
        "derived {} new filters; table 11 states {}",
        survey.new_by_namespace(Namespace::Filter),
        survey.new_filters
    );
    ensure!(
        survey.new_by_namespace(Namespace::Test) == survey.new_tests,
        "derived {} new tests; table 11 states {}",
        survey.new_by_namespace(Namespace::Test),
        survey.new_tests
    );
    ensure!(
        survey.new_rows.len() == survey.proposed,
        "derived {} new helpers; section 6.1 states {}",
        survey.new_rows.len(),
        survey.proposed
    );
    ensure!(
        survey.optioned.len() == survey.optioned_total,
        "derived {} existing helpers gaining an option; table 11 states {}",
        survey.optioned.len(),
        survey.optioned_total
    );
    ensure!(
        survey.accepted.len() == survey.new_rows.len() + survey.optioned.len(),
        "the accepted set has {} members, but {} new plus {} optioned is {}",
        survey.accepted.len(),
        survey.new_rows.len(),
        survey.optioned.len(),
        survey.new_rows.len() + survey.optioned.len()
    );
    Ok(())
}

/// Assert the deny set is what the complement rule says it is.
pub(super) fn check_denied(survey: &Survey) -> Result<()> {
    ensure!(
        survey.denied.len() == 71,
        "derived {} forbidden names; the complement rule gives 71",
        survey.denied.len()
    );
    let must_deny = [
        "is_file",
        "is_dir",
        "is_link",
        "quote",
        "fileglob",
        "lookup",
        "win_dirname",
        "expanduser",
    ];
    let missing = name_set(must_deny)
        .difference(&survey.denied)
        .cloned()
        .collect::<Vec<_>>();
    ensure!(
        missing.is_empty(),
        "the deny set omits {missing:?}, which section 7 or section 9 forbids"
    );
    let must_allow = [
        "basename",
        "dirname",
        "abs",
        "glob",
        "shell_quote",
        "splitdrive",
        "text_hash",
    ];
    let wrongly_denied = must_allow
        .iter()
        .filter(|name| survey.denied.contains(**name))
        .collect::<Vec<_>>();
    ensure!(
        wrongly_denied.is_empty(),
        "the deny set forbids {wrongly_denied:?}, which are accepted helpers the registries must \
         carry"
    );
    // Both sets, not just `accepted`: `hash` is a *surveyed* spelling whose
    // registered name is `text_hash`, so it is not in the accepted set by
    // construction. What the message below claims is that the surveyed spelling
    // reached neither set, and a future reject row naming `hash` would put it in
    // `denied` — which is the case a check on `accepted` alone cannot see.
    let hash_sets = [
        (survey.accepted.contains_key("hash"), "the accepted set"),
        (survey.denied.contains("hash"), "the deny set"),
    ];
    let present = hash_sets
        .iter()
        .filter_map(|(present, name)| present.then_some(*name))
        .collect::<Vec<_>>();
    ensure!(
        present.is_empty(),
        "`hash` is an existing Netsuke helper that RFC 0006 leaves unchanged, so it must be \
         neither accepted nor denied; it is in {}",
        present.join(" and ")
    );
    Ok(())
}
