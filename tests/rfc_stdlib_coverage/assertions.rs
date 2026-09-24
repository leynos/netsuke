//! Cross-checks between the derived sets and the words RFC 0006 states them in.
//!
//! These are part of the derivation rather than a separate check, because every
//! one of them compares two places the RFC states the same thing: the row counts
//! against table 11, the class counts against the reject-row count, the
//! new-filter and new-test counts against the tables, and the deny set against
//! the non-vacuity witnesses. A parse that silently returns nothing must fail
//! here.

use anyhow::{Result, ensure};

use super::{Namespace, name_set, survey::Survey};

/// Assert the derived totals agree with what RFC 0006 states.
pub(super) fn check_against_document(survey: &Survey) -> Result<()> {
    ensure!(
        survey.accept_rows == 55,
        "derived {} accept rows in RFC 0006 section 7; table 11 states 55",
        survey.accept_rows
    );
    ensure!(
        survey.defer_rows == 6,
        "derived {} defer rows in RFC 0006 section 7; table 11 states 6",
        survey.defer_rows
    );
    ensure!(
        survey.reject_rows == 50,
        "derived {} reject rows in RFC 0006 section 7; table 11 states 22 + 10 + 18",
        survey.reject_rows
    );
    ensure!(
        survey.reject_classes.iter().sum::<usize>() == survey.reject_rows,
        "table 11's class counts {:?} sum to {}, but section 7 has {} reject rows",
        survey.reject_classes,
        survey.reject_classes.iter().sum::<usize>(),
        survey.reject_rows
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
