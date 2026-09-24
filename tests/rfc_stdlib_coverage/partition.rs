//! The ownership partition: every accepted helper has exactly one owner.
//!
//! These two checks read the coverage map as a partition of RFC 0006's section 7
//! dispositions. `COV-1` asserts the map's rows cover the accepted set exactly
//! once, and `COV-2` asserts no child RFC registers a name the survey defers or
//! rejects. Together they are the mechanism the task exists to provide: a helper
//! cannot be dropped, double-assigned, or reintroduced under a rejected spelling
//! without a test naming the file and line.
//!
//! Split out of `checks.rs` to keep both modules under Whitaker's 400-line
//! `module_max_lines` ceiling. The seam is the derivation each check reads: both
//! start from a registry compared against the survey's dispositions, whereas the
//! checks in `super::progress` start from the map's own reported state.

use std::collections::BTreeSet;

use anyhow::{Result, ensure};

use super::{Registration, Repo, World, registries, survey};

/// Every accepted helper has exactly one owning child RFC, and the map's rows
/// together claim every accepted helper exactly once.
pub fn every_accepted_helper_has_exactly_one_owner(repo: &Repo) -> Result<()> {
    let world = World::load(repo)?;
    let ownership = world.map.ownership()?;

    let accepted: BTreeSet<String> = world.survey.accepted.keys().cloned().collect();
    let claimed: BTreeSet<String> = ownership.keys().cloned().collect();
    let unowned: Vec<_> = accepted.difference(&claimed).take(10).cloned().collect();
    ensure!(
        unowned.is_empty(),
        "the coverage map claims no owner for {unowned:?}; every accepted helper needs exactly one"
    );
    let extra: Vec<_> = claimed.difference(&accepted).take(10).cloned().collect();
    ensure!(
        extra.is_empty(),
        "the coverage map claims {extra:?}, which RFC 0006 does not accept"
    );

    // A written child's registry must match the rows that claim it.
    for registry in &world.registries {
        let Some(claimed_by_row) = world
            .map
            .rows
            .iter()
            .find(|row| row.number == registry.number)
        else {
            return Err(anyhow::anyhow!(
                "RFC {} exists at {} but the coverage map has no row for it",
                registry.number,
                registry.file
            ));
        };
        let expected: BTreeSet<String> = claimed_by_row.claims().into_iter().collect();
        let actual = registry.names();
        let missing: Vec<_> = expected.difference(&actual).take(10).cloned().collect();
        let unexpected: Vec<_> = actual.difference(&expected).take(10).cloned().collect();
        ensure!(
            missing.is_empty() && unexpected.is_empty(),
            "RFC {}'s registry does not match its coverage map row: missing {missing:?}; \
             unexpected {unexpected:?}",
            registry.number
        );
        // A matching name set is not a matching registry. The namespace and the
        // registration kind are parsed from the child's own row, so without
        // this comparison a row could move a helper to the wrong namespace or
        // mark an optioned helper as new and still pass every check — and the
        // namespace is exactly what `COV-3`'s filter and test totals count.
        check_rows_agree_with_survey(registry, &world.survey)?;
    }
    Ok(())
}

/// Each registry row's namespace and registration agree with RFC 0006.
///
/// The survey is the source of truth on both: its `accepted` rows carry the
/// namespace section 7 assigns the helper, and `optioned` lists the three names
/// that gain an option rather than being introduced. Comparing the two makes
/// the registry columns load-bearing rather than decorative.
fn check_rows_agree_with_survey(
    registry: &registries::Registry,
    survey: &survey::Survey,
) -> Result<()> {
    for row in &registry.rows {
        let name = &row.helper.name;
        let Some(accepted) = survey.accepted.get(name) else {
            return Err(anyhow::anyhow!(
                "{} registers {name}, which RFC 0006 section 7 does not accept",
                registry.file
            ));
        };
        ensure!(
            accepted.namespace == row.helper.namespace,
            "{} gives {name} namespace {}, but RFC 0006 section 7 places it in {}",
            registry.file,
            row.helper.namespace.label(),
            accepted.namespace.label()
        );
        let optioned = survey.optioned.contains(name);
        let expected = if optioned {
            Registration::OptionAdded
        } else {
            Registration::New
        };
        ensure!(
            row.registration == expected,
            "{} marks {name} `{}`, but RFC 0006 {} it `{}`",
            registry.file,
            row.registration.label(),
            if optioned { "lists" } else { "introduces" },
            expected.label()
        );
    }
    Ok(())
}

/// No child RFC registers a name RFC 0006 defers or rejects.
pub fn no_forbidden_helper_is_registered(repo: &Repo) -> Result<()> {
    let world = World::load(repo)?;
    let mut violations = Vec::new();
    for registry in &world.registries {
        for name in registry.names() {
            if world.survey.denied.contains(&name) {
                violations.push(format!(
                    "{} registers {name}, which RFC 0006 section 7 or section 9 forbids",
                    registry.file
                ));
            }
        }
    }
    ensure!(
        violations.is_empty(),
        "forbidden helpers registered: {violations:?}"
    );
    Ok(())
}
