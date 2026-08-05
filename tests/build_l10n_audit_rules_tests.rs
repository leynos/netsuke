//! Tests for the localization audit's comparison rules.
//!
//! Split from `build_l10n_parser_tests.rs` to keep both files within the
//! repository's 400-line limit. That file covers the parsers that read the
//! catalogues and the Cargo metadata; this one covers what the audit does with
//! their results — which keys are missing, orphaned, or interpolate the wrong
//! variables, and how the resulting failure reads.

use std::collections::BTreeSet;

use anyhow::{Result, anyhow, ensure};
use rstest::rstest;

#[path = "../build_l10n_audit/compare.rs"]
mod compare;
#[path = "../build_l10n_audit/ftl.rs"]
mod ftl;

/// Build a `MessageVariables` map from `(key, variables)` pairs.
fn catalogue(entries: &[(&str, &[&str])]) -> ftl::MessageVariables {
    entries
        .iter()
        .map(|(key, vars)| {
            (
                (*key).to_owned(),
                vars.iter().map(|v| (*v).to_owned()).collect(),
            )
        })
        .collect()
}

fn declared(keys: &[&str]) -> BTreeSet<String> {
    keys.iter().map(|key| (*key).to_owned()).collect()
}

/// Audit `entries` for a locale against a one-key source, returning the
/// failure message, or `None` when the catalogue is clean.
fn audit(
    declared_keys: &[&str],
    source: &[(&str, &[&str])],
    entries: &[(&str, &[&str])],
) -> Option<String> {
    let findings = compare::audit_catalogue(
        "xx",
        &declared(declared_keys),
        &catalogue(source),
        &catalogue(entries),
    );
    (!findings.is_clean()).then(|| compare::build_error_message(std::slice::from_ref(&findings)))
}

const SOURCE: &[(&str, &[&str])] = &[("a.key", &["path"]), ("b.key", &[])];
const DECLARED: &[&str] = &["a.key", "b.key"];

/// A catalogue matching the declared keys and the source variables passes.
#[test]
fn a_matching_catalogue_is_clean() -> Result<()> {
    let message = audit(DECLARED, SOURCE, SOURCE);
    ensure!(message.is_none(), "expected no findings, got {message:?}");
    Ok(())
}

/// Catalogues that each break one audit rule against `SOURCE`.
const OMITS_A_DECLARED_KEY: &[(&str, &[&str])] = &[("a.key", &["path"])];
const CARRIES_AN_UNDECLARED_KEY: &[(&str, &[&str])] =
    &[("a.key", &["path"]), ("b.key", &[]), ("c.key", &[])];
const DROPS_A_VARIABLE: &[(&str, &[&str])] = &[("a.key", &[]), ("b.key", &[])];
const INVENTS_A_VARIABLE: &[(&str, &[&str])] = &[("a.key", &["path"]), ("b.key", &["name"])];
const RENAMES_A_VARIABLE: &[(&str, &[&str])] = &[("a.key", &["route"]), ("b.key", &[])];

#[rstest]
#[case(OMITS_A_DECLARED_KEY, "missing in xx: b.key")]
#[case(CARRIES_AN_UNDECLARED_KEY, "orphaned in xx: c.key")]
#[case(
    DROPS_A_VARIABLE,
    "variable mismatch in xx: a.key (expected $path, found none)"
)]
#[case(
    INVENTS_A_VARIABLE,
    "variable mismatch in xx: b.key (expected none, found $name)"
)]
#[case(
    RENAMES_A_VARIABLE,
    "variable mismatch in xx: a.key (expected $path, found $route)"
)]
fn the_audit_rejects(#[case] entries: &[(&str, &[&str])], #[case] expected: &str) -> Result<()> {
    let message = audit(DECLARED, SOURCE, entries)
        .ok_or_else(|| anyhow!("expected the audit to report a finding"))?;
    ensure!(
        message.contains(expected),
        "expected a finding mentioning {expected:?}, got {message:?}"
    );
    Ok(())
}

/// One catalogue can fail several rules at once, and the message names each.
#[test]
fn every_rule_is_reported_together() -> Result<()> {
    const BREAKS_EVERY_RULE: &[(&str, &[&str])] = &[("a.key", &[]), ("c.key", &[])];
    let entries = BREAKS_EVERY_RULE;
    let message = audit(DECLARED, SOURCE, entries)
        .ok_or_else(|| anyhow!("expected the audit to report findings"))?;
    for expected in [
        "missing in xx: b.key",
        "orphaned in xx: c.key",
        "variable mismatch in xx: a.key",
    ] {
        ensure!(
            message.contains(expected),
            "expected {expected:?} in {message:?}"
        );
    }
    Ok(())
}

/// The audit's failure message is user-visible build output, so its exact shape
/// is pinned rather than probed a substring at a time.
///
/// One locale carrying all three categories at once is the case a substring
/// assertion covers least well: it says nothing about ordering, grouping, or
/// how the sections read together, which is what a maintainer actually sees
/// when a build fails. The inputs are fixed literals and every collection the
/// message renders is a `BTree*`, so the output is deterministic — no paths,
/// temporary directories, or error wrappers appear in it.
#[test]
fn the_failure_message_reports_every_category() -> Result<()> {
    const SOURCE_KEYS: &[(&str, &[&str])] = &[("a.key", &["path"]), ("b.key", &["count"])];
    // Drops `a.key`, renames `b.key`'s variable, and adds an undeclared key.
    const DRIFTED: &[(&str, &[&str])] = &[("b.key", &["tally"]), ("z.orphan", &[])];

    let message = audit(&["a.key", "b.key"], SOURCE_KEYS, DRIFTED)
        .ok_or_else(|| anyhow!("expected the drifted catalogue to be rejected"))?;

    // Kept alongside the snapshot: these say which categories must appear, so
    // an accidental snapshot acceptance cannot quietly drop one.
    ensure!(
        message.contains("missing in xx: a.key"),
        "expected the missing key, got {message}"
    );
    ensure!(
        message.contains("orphaned in xx: z.orphan"),
        "expected the orphaned key, got {message}"
    );
    ensure!(
        message.contains("variable mismatch in xx: b.key"),
        "expected the variable mismatch, got {message}"
    );

    insta::assert_snapshot!(message);
    Ok(())
}

/// The parser and the rules compose: a catalogue read from FTL text is audited
/// the same way one built by hand is.
///
/// The other tests here construct `MessageVariables` directly, which keeps them
/// focused on the rules but leaves the seam between the two halves untested.
/// This drives real catalogue text through `ftl::parse_catalogue` and into
/// `audit_catalogue`, so a change to how variables are collected shows up as an
/// audit result rather than only as a parser result.
#[test]
fn a_parsed_catalogue_is_audited_by_the_same_rules() -> Result<()> {
    let source = ftl::parse_catalogue("a.key = Uses { $path }\nb.key = Plain text\n")
        .map_err(|error| anyhow!("{error}"))?;
    // Renames the variable and drops `b.key`, adding an undeclared key instead.
    let drifted = ftl::parse_catalogue("a.key = Utilise { $chemin }\nz.orphan = Extra\n")
        .map_err(|error| anyhow!("{error}"))?;

    let findings =
        compare::audit_catalogue("fr", &declared(&["a.key", "b.key"]), &source, &drifted);
    ensure!(
        !findings.is_clean(),
        "the drifted catalogue must be rejected"
    );

    let message = compare::build_error_message(std::slice::from_ref(&findings));
    ensure!(
        message.contains("missing in fr: b.key"),
        "expected the dropped key, got {message}"
    );
    ensure!(
        message.contains("orphaned in fr: z.orphan"),
        "expected the undeclared key, got {message}"
    );
    ensure!(
        message.contains("variable mismatch in fr: a.key (expected $path, found $chemin)"),
        "expected the renamed variable, got {message}"
    );
    Ok(())
}
