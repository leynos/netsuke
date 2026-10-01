//! Validate partial budgets, complete equalities and reported delivery state.

use super::*;
use crate::rfc_stdlib_coverage::validation_fixtures as fixture;
use rstest::rstest;

#[test]
fn accepts_partial_and_complete_exact_totals() -> Result<()> {
    let mut world = fixture::world();
    check_totals(&world)?;
    world
        .map
        .rows
        .iter_mut()
        .for_each(|row| row.is_written = true);
    check_totals(&world)
}

#[rstest]
#[case::partial_helpers("partial_helpers", "accepts only 0")]
#[case::complete_helpers("complete_helpers", "RFC 0006 accepts 2")]
#[case::clock("clock", "helper(s) `clock-observing`")]
#[case::network("network", "helper(s) `network-observing`")]
#[case::subprocess("subprocess", "helper(s) `subprocess-observing`")]
#[case::pure_budget("pure_budget", "states only 0/0/0 in total")]
#[case::filesystem_budget("filesystem_budget", "declare 0 pure / 1 filesystem / 0 environment")]
#[case::environment_budget("environment_budget", "declare 0 pure / 0 filesystem / 1 environment")]
#[case::option_budget("option_budget", "table 11 lists only 0")]
#[case::pure_equality("pure_equality", "purity aggregate is 1 pure / 0 filesystem")]
#[case::filesystem_equality("filesystem_equality", "states 1/1/0")]
#[case::environment_equality("environment_equality", "states 1/0/1")]
#[case::option_equality("option_equality", "table 11 states 1")]
fn rejects_total_or_purity_mismatches(#[case] mutation: &str, #[case] diagnostic: &str) {
    let mut world = fixture::world();
    if mutation.starts_with("complete") || mutation.ends_with("equality") {
        world
            .map
            .rows
            .iter_mut()
            .for_each(|row| row.is_written = true);
    }
    match mutation {
        "partial_helpers" => world.survey.accepted.clear(),
        "complete_helpers" => {
            world
                .survey
                .accepted
                .insert("other".into(), fixture::helper("other"));
        }
        "clock" => {
            world
                .registries
                .first_mut()
                .expect("fixture registry")
                .rows
                .first_mut()
                .expect("fixture row")
                .purity = registries::Purity::Clock;
        }
        "network" => {
            world
                .registries
                .first_mut()
                .expect("fixture registry")
                .rows
                .first_mut()
                .expect("fixture row")
                .purity = registries::Purity::Network;
        }
        "subprocess" => {
            world
                .registries
                .first_mut()
                .expect("fixture registry")
                .rows
                .first_mut()
                .expect("fixture row")
                .purity = registries::Purity::Subprocess;
        }
        "pure_budget" => world.survey.purity.0 = 0,
        "filesystem_budget" => {
            world
                .registries
                .first_mut()
                .expect("fixture registry")
                .rows
                .first_mut()
                .expect("fixture row")
                .purity = registries::Purity::Filesystem;
        }
        "environment_budget" => {
            world
                .registries
                .first_mut()
                .expect("fixture registry")
                .rows
                .first_mut()
                .expect("fixture row")
                .purity = registries::Purity::Environment;
        }
        "option_budget" => {
            world
                .registries
                .first_mut()
                .expect("fixture registry")
                .rows
                .first_mut()
                .expect("fixture row")
                .registration = Registration::OptionAdded;
        }
        "pure_equality" => world.survey.purity.0 = 2,
        "filesystem_equality" => world.survey.purity.1 = 1,
        "environment_equality" => world.survey.purity.2 = 1,
        "option_equality" => world.survey.optioned.push("optioned".into()),
        _ => {}
    }
    let error = check_totals(&world).expect_err("one aggregate condition changed");
    assert!(error.to_string().contains(diagnostic), "{error:#}");
}

#[test]
fn optioned_rows_do_not_consume_new_helper_purity_budgets() -> Result<()> {
    let mut world = fixture::world();
    world
        .registries
        .first_mut()
        .expect("fixture registry")
        .rows
        .first_mut()
        .expect("fixture row")
        .registration = Registration::OptionAdded;
    world
        .registries
        .first_mut()
        .expect("fixture registry")
        .rows
        .first_mut()
        .expect("fixture row")
        .purity = registries::Purity::Filesystem;
    world.survey.optioned.push("helper".into());
    world.survey.purity = (0, 0, 0);
    check_registry_aggregate(&world)
}

#[rstest]
#[case::unscheduled("unscheduled", "helper (RFC 0013, step 6.2)")]
#[case::unclaimed("unclaimed", "helper (claimed by no coverage-map row)")]
#[case::wrong_step("wrong_step", "step 6.2, but that step names none of [\"helper\"]")]
#[case::unknown_step("unknown_step", "roadmap step 6.11 does not exist")]
fn rejects_missing_or_misplaced_scheduled_helpers(
    #[case] mutation: &str,
    #[case] diagnostic: &str,
) {
    let mut world = fixture::world();
    match mutation {
        "unscheduled" => {
            world
                .roadmap
                .names
                .get_mut("6.2")
                .expect("fixture step")
                .clear();
        }
        "unclaimed" => {
            world
                .roadmap
                .names
                .get_mut("6.2")
                .expect("fixture step")
                .clear();
            world
                .map
                .rows
                .first_mut()
                .expect("fixture owning map row")
                .owns
                .clear();
        }
        "wrong_step" => {
            world
                .roadmap
                .names
                .get_mut("6.2")
                .expect("fixture step")
                .clear();
            world
                .roadmap
                .names
                .get_mut("6.3")
                .expect("fixture step")
                .insert("helper".into());
        }
        "unknown_step" => {
            world
                .map
                .rows
                .first_mut()
                .expect("fixture owning map row")
                .step = "6.11".into();
        }
        _ => {}
    }
    let error = check_schedule(&world).expect_err("one scheduling condition changed");
    assert!(error.to_string().contains(diagnostic), "{error:#}");
}

#[rstest]
#[case::count("count", "coverage map has 7 rows; expected 8")]
#[case::escape("escape", "links to ../../../outside.md, which climbs above")]
#[case::missing_file(
    "missing_file",
    "resolves to docs/rfcs/missing.md; no such file exists"
)]
#[case::unwritten("unwritten", "coverage map row still says unwritten")]
#[case::unparsed("unparsed", "marked written, but no registry was parsed for it")]
fn rejects_dishonest_progress(#[case] mutation: &str, #[case] diagnostic: &str) -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let root = camino::Utf8Path::from_path(temporary.path()).context("UTF-8 fixture root")?;
    let repo = Repo::fixture(root)?;
    let mut world = fixture::world();
    match mutation {
        "count" => {
            world.map.rows.pop();
        }
        "escape" => {
            world
                .child_paths
                .insert("0013".into(), "../../../outside.md".into());
        }
        "missing_file" => {
            world.child_paths.insert("0013".into(), "missing.md".into());
        }
        "unwritten" => {
            world
                .map
                .rows
                .first_mut()
                .expect("fixture owning map row")
                .is_written = false;
        }
        "unparsed" => world.registries.clear(),
        _ => {}
    }
    let error = check_status(&repo, &world).expect_err("one progress condition changed");
    ensure!(error.to_string().contains(diagnostic), "{error:#}");
    Ok(())
}

#[test]
fn accepts_a_written_row_linking_to_an_existing_parsed_child() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let root = camino::Utf8Path::from_path(temporary.path()).context("UTF-8 fixture root")?;
    let dir = cap_std::fs_utf8::Dir::open_ambient_dir(root, cap_std::ambient_authority())?;
    dir.create_dir_all("docs/rfcs")?;
    dir.write("docs/rfcs/0013-child.md", "fixture")?;
    let mut world = fixture::world();
    world
        .child_paths
        .insert("0013".into(), "0013-child.md#registry".into());
    check_status(&Repo::fixture(root)?, &world)?;
    check_schedule(&world)
}

#[rstest]
#[case::missing("missing", "does not discharge RFC 0006 clause(s) [\"6.1\"]")]
#[case::invented("invented", "discharges [\"6.2\"], which is not a clause")]
#[case::section_mismatch("section_mismatch", "the two must name the same clauses")]
fn rejects_inconsistent_clause_sets(
    #[case] mutation: &str,
    #[case] diagnostic: &str,
) -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let root = camino::Utf8Path::from_path(temporary.path()).context("UTF-8 fixture root")?;
    let dir = cap_std::fs_utf8::Dir::open_ambient_dir(root, cap_std::ambient_authority())?;
    dir.create_dir_all("docs/rfcs")?;
    dir.write(
        super::super::RFC_0006,
        "## 6. Cross-cutting contract\n### 6.1. Purity\nContract.\n",
    )?;
    let child = concat!(
        "## 5. Cross-cutting contract conformance\n",
        "### 5.1. Registry\n`helper` has a group-specific obligation.\n",
        "### Clause discharge\n| Clause | Discharge |\n| --- | --- |\n| `6.1` | Met |\n"
    );
    let mutated_child = match mutation {
        "missing" => child.replace("`6.1`", "`6.2`"),
        "invented" => format!("{child}| `6.2` | Met |\n"),
        "section_mismatch" => child.replace("### 5.1.", "### 5.2."),
        _ => child.into(),
    };
    let world = fixture::world();
    dir.write(
        &world.registries.first().context("fixture registry")?.file,
        mutated_child,
    )?;
    let error = check_discharges(&Repo::fixture(root)?, &world).expect_err("clause mismatch");
    ensure!(error.to_string().contains("docs/rfcs/0013-child.md"));
    ensure!(error.to_string().contains(diagnostic), "{error:#}");
    Ok(())
}

#[test]
fn inter_document_check_reports_corpus_failures() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let root = camino::Utf8Path::from_path(temporary.path()).context("UTF-8 fixture root")?;
    let dir = cap_std::fs_utf8::Dir::open_ambient_dir(root, cap_std::ambient_authority())?;
    dir.create_dir_all("docs/rfcs")?;
    dir.write("docs/rfcs/0013-child.md", "[missing](missing.md)")?;
    let error = inter_document_links_resolve(&Repo::fixture(root)?).expect_err("missing document");
    ensure!(error.to_string().contains("dangling inter-document links"));
    ensure!(error.to_string().contains("docs/rfcs/0013-child.md:1"));
    Ok(())
}
