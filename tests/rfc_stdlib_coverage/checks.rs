//! The seven coverage checks RFC 0006's split is contracted against.
//!
//! Each is a test entry point named for the obligation it discharges. They share
//! one derivation — RFC 0006 section 7 for the accepted and deny sets, section
//! 14.13 for the coverage map, the child RFCs' registries, the roadmap, and the
//! link graph — so a single parse bug surfaces in whichever check depends on the
//! part it corrupts.
//!
//! Three of the seven are vacuously true until a child RFC exists. That is not a
//! reason to defer them: their non-vacuity is supplied by the seeded-fault
//! controls the `ExecPlan` records, which run against scratch copies at each
//! plateau. A check written only once its subject exists is a check whose parser
//! has never been exercised.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use super::{Repo, clauses, links, map, registries, roadmap, survey};

/// Context every check needs, derived once per call.
struct World {
    /// The parse of RFC 0006's dispositions and totals.
    pub(super) survey: survey::Survey,
    /// The parse of RFC 0006's coverage map.
    pub(super) map: map::Map,
    /// Every existing child RFC's registry.
    pub(super) registries: Vec<registries::Registry>,
    /// The roadmap's capability steps.
    pub(super) roadmap: roadmap::Steps,
    /// The failure path each child RFC's number resolves to, when written.
    pub(super) child_paths: std::collections::BTreeMap<String, String>,
}

impl World {
    /// Derive everything from the working tree.
    pub(super) fn load(repo: &Repo) -> Result<Self> {
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

/// The derived totals and the registries' purity aggregate agree with RFC 0006.
pub fn totals_and_purity_aggregate_agree(repo: &Repo) -> Result<()> {
    let world = World::load(repo)?;

    let registered: usize = world
        .registries
        .iter()
        .map(|registry| registry.names().len())
        .sum();
    let written = world.map.rows.len() - world.map.unwritten();
    if world.map.unwritten() == 0 {
        ensure!(
            registered == world.survey.accepted.len(),
            "the registries together list {registered} helpers; RFC 0006 accepts {}",
            world.survey.accepted.len()
        );
    }

    // The purity aggregate is taken over `New` rows only, because section 6.1's
    // 52/4/1 counts the 57 proposed helpers. The registries carry all 60
    // accepted helpers, and the optioned rows include the filesystem-observing
    // `glob`, so an all-row aggregate would be 54/5/1.
    if written == world.map.rows.len() {
        let pure: usize = world
            .registries
            .iter()
            .map(|registry| registry.with_purity(registries::Purity::Pure))
            .sum();
        let filesystem: usize = world
            .registries
            .iter()
            .map(|registry| registry.with_purity(registries::Purity::Filesystem))
            .sum();
        let environment: usize = world
            .registries
            .iter()
            .map(|registry| registry.with_purity(registries::Purity::Environment))
            .sum();
        let (want_pure, want_filesystem, want_environment) = world.survey.purity;
        ensure!(
            (pure, filesystem, environment) == (want_pure, want_filesystem, want_environment),
            "the registries' purity aggregate is {pure} pure / {filesystem} filesystem / \
             {environment} environment; RFC 0006 section 6.1 states {want_pure}/{want_filesystem}/\
             {want_environment}"
        );
        let optioned: usize = world
            .registries
            .iter()
            .map(|registry| registry.with_registration(super::Registration::OptionAdded))
            .sum();
        ensure!(
            optioned == world.survey.optioned.len(),
            "the registries mark {optioned} rows `option added`; RFC 0006 table 11 states {}",
            world.survey.optioned.len()
        );
    }
    Ok(())
}

/// The coverage map reports progress honestly.
///
/// The count is printed rather than only asserted, because a half-finished
/// split satisfies every other obligation: the abandoned state and the finished
/// state are indistinguishable to a bijection check, so the one thing that
/// announces a stall is the number of groups still unwritten. `nextest.toml`
/// raises this test's success output to `immediate` so the line reaches the
/// terminal on a green run rather than being captured.
#[expect(
    clippy::print_stdout,
    reason = "obligation COV-4 requires the unwritten-group count in the passing \
              test's output, and this target prints nowhere else"
)]
pub fn coverage_map_status_is_reported(repo: &Repo) -> Result<()> {
    let world = World::load(repo)?;
    let unwritten = world.map.unwritten();
    println!(
        "coverage map: {} of {} capability groups written; {unwritten} remaining",
        world.map.rows.len() - unwritten,
        world.map.rows.len()
    );
    ensure!(
        world.map.rows.len() == 8,
        "the coverage map has {} rows; expected 8",
        world.map.rows.len()
    );
    // Every accepted helper the map claims must have an owning row, and every
    // written row must have a child file that exists. `parse` already checked
    // that a written row carries a link; here the link is resolved.
    for row in &world.map.rows {
        if let Some(target) = &world.child_paths.get(&row.number) {
            let path = links::resolve(super::RFC_DIR, target).with_context(|| {
                format!(
                    "coverage map row for RFC {} links to {target}, which climbs above the \
                     repository root",
                    row.number
                )
            })?;
            ensure!(
                repo.exists(&path)?,
                "coverage map row for RFC {} is marked written and links to {target}, which \
                 resolves to {path}; no such file exists",
                row.number
            );
        }
    }
    for registry in &world.registries {
        let matched = world
            .map
            .rows
            .iter()
            .find(|row| row.number == registry.number);
        ensure!(
            matched.is_none_or(|row| row.is_written),
            "RFC {} exists at {} but its coverage map row still says unwritten",
            registry.number,
            registry.file
        );
    }
    Ok(())
}

/// Every relative link in the RFC corpus resolves.
pub fn inter_document_links_resolve(repo: &Repo) -> Result<()> {
    let failures = links::dangling(repo)?;
    ensure!(
        failures.is_empty(),
        "dangling inter-document links: {failures:?}"
    );
    Ok(())
}

/// Every capability has a roadmap task, and each child's step names what it owns.
pub fn every_capability_has_a_roadmap_task(repo: &Repo) -> Result<()> {
    let world = World::load(repo)?;
    let unscheduled = world.roadmap.unscheduled(world.survey.accepted.keys());
    ensure!(
        unscheduled.is_empty(),
        "accepted helpers {:?} are named in no roadmap capability step at all, so nothing \
         schedules them",
        // Name the step each one belongs to. A helper named nowhere is exactly
        // the case where the reader needs to be told where it was meant to go,
        // and the coverage map is the only place that records it.
        unscheduled
            .iter()
            .map(|name| {
                world
                    .map
                    .rows
                    .iter()
                    .find(|row| row.claims().contains(name))
                    .map_or_else(
                        || format!("{name} (claimed by no coverage-map row)"),
                        |row| format!("{name} (RFC {}, step {})", row.number, row.step),
                    )
            })
            .collect::<Vec<_>>()
    );
    for row in &world.map.rows {
        let missing = world
            .roadmap
            .missing_from_step(&row.step, row.claims().iter())?;
        ensure!(
            missing.is_empty(),
            "the coverage map gives RFC {} roadmap step {}, but that step names none of {missing:?}",
            row.number,
            row.step
        );
    }
    Ok(())
}

/// Every child RFC discharges every clause of RFC 0006 section 6.
pub fn every_child_discharges_every_clause(repo: &Repo) -> Result<()> {
    let world = World::load(repo)?;
    let clauses: BTreeSet<String> = clauses::clause_ids(repo)?.into_iter().collect();
    for registry in &world.registries {
        let discharged = clauses::discharged(repo, &registry.file)?;
        let undischarged: Vec<_> = clauses.difference(&discharged).cloned().collect();
        ensure!(
            undischarged.is_empty(),
            "{} does not discharge RFC 0006 clause(s) {undischarged:?}",
            registry.file
        );
        let invented: Vec<_> = discharged.difference(&clauses).cloned().collect();
        ensure!(
            invented.is_empty(),
            "{} discharges {invented:?}, which is not a clause of RFC 0006 section 6",
            registry.file
        );
    }
    Ok(())
}
